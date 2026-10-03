import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import type {
  CapturePreview,
  InboxPage,
  LibraryDetail,
  LibraryFilters,
  LibraryPage,
  ReadingGroup,
  ScenarioSummary,
  SetInboxStatusRequest,
  SolveResult,
  TodayView,
} from '@/lib/contracts';
import type { AutomationStatus } from '@/lib/automation-view';
import type { GitHubAuthState } from '@/types';

/**
 * 新界面用到的后端命令。类型来自契约。
 * 测试用 setInvokeForTests 换掉真实调用，不连 App。
 */

export type PackRole = 'main' | 'alternative' | 'inspiration';
export type PackFormat = 'markdown' | 'agents' | 'json';

export type PackItemInput = {
  repoId: string;
  role: PackRole;
  why?: string;
};

export type LocalCapabilityInput = {
  name: string;
  oneLiner: string;
  path: string;
};

export type PackRequest = {
  title: string;
  problem: string;
  architectureNotes?: string;
  items: PackItemInput[];
  localCapabilities: LocalCapabilityInput[];
};

export type PackOutput = {
  format: PackFormat;
  content: string;
  itemCount: number;
  missing: string[];
  warnings: string[];
};

/** 与 pages/v2/capture.tsx 里的 Commit 同形。 */
export type CaptureCommit = {
  created: number;
  skipped: number;
  duplicateBatch: boolean;
  alreadyStarred: number;
  alreadyInInbox: number;
  blocked: number;
  linksFound: number;
};

type InvokeFn = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const realInvoke: InvokeFn = (command, args) => tauriInvoke(command, args);

let invokeImpl: InvokeFn = realInvoke;

export function setInvokeForTests(fn: InvokeFn | null): void {
  invokeImpl = fn ?? realInvoke;
}

function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return invokeImpl(command, args) as Promise<T>;
}

function definedArgs(values: Record<string, unknown>): Record<string, unknown> {
  const args: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(values)) {
    if (value !== undefined) args[key] = value;
  }
  return args;
}

export function listScenarios(): Promise<ScenarioSummary[]> {
  return call<ScenarioSummary[]>('list_scenarios');
}

export function listLibraryView(filters: LibraryFilters): Promise<LibraryPage> {
  return call<LibraryPage>('list_library_view', { filters });
}

export function getLibraryDetail(id: string): Promise<LibraryDetail> {
  return call<LibraryDetail>('get_library_detail', { id });
}

export function setLibraryReading(ids: string[], group: ReadingGroup): Promise<number> {
  return call<number>('set_library_reading', { ids, group });
}

export function getTodayView(): Promise<TodayView> {
  return call<TodayView>('get_today_view');
}

export function solveV2(query: string): Promise<SolveResult> {
  return call<SolveResult>('solve_v2', { query });
}

export function buildReferencePack(
  request: PackRequest,
  format: PackFormat,
  generatedOn?: string,
): Promise<PackOutput> {
  return call<PackOutput>('build_reference_pack_v2', {
    request,
    format,
    generatedOn: generatedOn ?? null,
  });
}

export function copyItemForCodex(id: string): Promise<string> {
  return call<string>('copy_item_for_codex', { repoId: id });
}

export function previewCapture(text: string): Promise<CapturePreview> {
  return call<CapturePreview>('preview_capture', { text });
}

export function listInbox(status?: string, limit?: number, offset?: number): Promise<InboxPage> {
  return call<InboxPage>('list_inbox', definedArgs({ status, limit, offset }));
}

export function setInboxStatus(id: string, status: SetInboxStatusRequest['status']): Promise<null> {
  return call<null>('set_inbox_status', { id, status });
}

export function setInboxNote(id: string, note: string): Promise<null> {
  return call<null>('set_inbox_note', { id, note });
}

export function confirmLibraryCard(id: string): Promise<null> {
  return call<null>('confirm_library_card', { id });
}

export function regenerateLibraryCard(id: string): Promise<null> {
  return call<null>('regenerate_library_card', { id });
}

export function commitCaptureBatch(text: string, sourceContext: string): Promise<CaptureCommit> {
  return call<CaptureCommit>('commit_capture_batch', { text, sourceContext });
}

export type RepositoryReadmeText = { rawMarkdown: string; sourcePath: string };

/** 读仓库缓存的 README 原文（现有命令 get_repository_detail）。没有缓存返回 null。账号 id 就是仓库 id 冒号前面那段。 */
export async function getRepositoryReadme(repositoryId: string): Promise<RepositoryReadmeText | null> {
  const accountId = repositoryId.split(':')[0] ?? '';
  const view = await call<{ readme?: { rawMarkdown?: string; sourcePath?: string } | null } | null>('get_repository_detail', {
    request: { repositoryId, accountId },
  });
  const raw = view?.readme?.rawMarkdown;
  if (typeof raw !== 'string' || !raw.trim()) return null;
  return { rawMarkdown: raw, sourcePath: view?.readme?.sourcePath ?? 'README.md' };
}

/** AI 自动整理现在的状态（预算、队列、有没有暂停）。只读，不花钱。 */
export function getAiAutomation(): Promise<AutomationStatus> {
  return call<AutomationStatus>('get_ai_automation');
}

export function getGithubAuthState(): Promise<GitHubAuthState> {
  return call<GitHubAuthState>('get_github_auth_state');
}

/** 把后端报错收成 Fox 能看懂的一句话。命令还没合并时，告诉他功能还没接上。 */
export function describeInvokeError(reason: unknown): string {
  const raw = readErrorMessage(reason).trim();
  if (isMissingCommand(raw)) return '这个功能还没接上。';
  const cleaned = raw.replace(/^Error:\s*/i, '').trim();
  if (!cleaned) return '操作没有完成，请稍后再试。';
  const chars = Array.from(cleaned);
  if (chars.length > 180) return `${chars.slice(0, 180).join('')}…`;
  return cleaned;
}

function readErrorMessage(reason: unknown): string {
  if (typeof reason === 'string') return reason;
  if (reason instanceof Error) return reason.message;
  if (reason && typeof reason === 'object' && 'message' in reason) {
    const message = (reason as { message?: unknown }).message;
    if (typeof message === 'string') return message;
  }
  if (reason == null) return '';
  return String(reason);
}

function isMissingCommand(raw: string): boolean {
  return /not found/i.test(raw)
    || raw.includes('没有这个命令')
    || raw.includes('命令不存在')
    || raw.includes('未知命令');
}
