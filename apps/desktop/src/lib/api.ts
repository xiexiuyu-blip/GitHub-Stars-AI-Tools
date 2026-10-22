import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import type {
  CapturePreview,
  DiscoverRefreshReport,
  DiscoverView,
  FetchSummary,
  InboxPage,
  LibraryDetail,
  LibraryFilters,
  LibraryPage,
  ReadingGroup,
  RepoCommunity,
  ScenarioSummary,
  SetInboxStatusRequest,
  RerankResult,
  SolveResult,
  TodayView,
} from '@/lib/contracts';
import { withRecentDone, type AiQueueStatus, type AutomationStatus } from '@/lib/automation-view';
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
  /** 不传就是用收藏里的工具。direct 是没有合适工具、直接交给 Codex。 */
  mode?: 'useTools' | 'direct';
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

export function solveRerankV2(request: { query: string; candidateIds: string[] }): Promise<RerankResult> {
  return call<RerankResult>('solve_rerank_v2', { request });
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

/** `problem` 是这次想做的事（找方案里那句话）：带上，Codex 才知道装这个工具是为了做什么。 */
export function copyItemForCodex(id: string, problem?: string): Promise<string> {
  return call<string>('copy_item_for_codex', definedArgs({ repoId: id, problem: problem?.trim() || undefined }));
}

export function previewCapture(text: string): Promise<CapturePreview> {
  return call<CapturePreview>('preview_capture', { text });
}

export function listInbox(status?: string, limit?: number, offset?: number): Promise<InboxPage> {
  return call<InboxPage>('list_inbox', definedArgs({ status, limit, offset }));
}

export function fetchInboxPending(id?: string): Promise<FetchSummary> {
  return call<FetchSummary>('fetch_inbox_pending', definedArgs({ id }));
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

export function regenerateLibraryCard(id: string, confirmed: boolean): Promise<null> {
  return call<null>('regenerate_library_card', { id, confirmed });
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

/** 发现：某个场景下缓存的候选。只读本地缓存，瞬间返回。scenarioId 为空时后端挑「偏少」排第一的场景。 */
export function listDiscover(scenarioId?: string | null): Promise<DiscoverView> {
  return call<DiscoverView>('list_discover', { scenarioId: scenarioId ?? null });
}

/** 发现：联网去 GitHub 搜这个场景（不登录，几秒到十几秒），结果写进缓存。 */
export function refreshDiscover(scenarioId: string): Promise<DiscoverRefreshReport> {
  return call<DiscoverRefreshReport>('refresh_discover', { scenarioId });
}

/** 社区：别人怎么用。refresh 为 false 只读缓存；为 true 才联网（Hacker News + GitHub Issues）。 */
export function getRepoCommunity(id: string, refresh = false): Promise<RepoCommunity> {
  return call<RepoCommunity>('get_repo_community', { id, refresh });
}

/** 本机已装的 Skill 和 MCP（只读名字和一句话，不读密钥）。后台还没有这个命令时返回空列表。 */
export async function scanLocalCapabilities(): Promise<LocalCapabilityInput[]> {
  try {
    const list = await call<LocalCapabilityInput[] | null>('scan_local_capabilities');
    return Array.isArray(list) ? list : [];
  } catch {
    return [];
  }
}

/** AI 自动整理现在的状态（预算、队列、有没有暂停）。只读，不花钱。 */
export function getAiAutomation(): Promise<AutomationStatus> {
  return call<AutomationStatus>('get_ai_automation');
}

/** 自动整理队列的状态（现成命令）。界面只用它的 `succeeded24h`：最近 24 小时整理成功的个数。只读，不花钱。 */
export function getAiQueueStatus(): Promise<AiQueueStatus> {
  return call<AiQueueStatus>('get_ai_queue_status');
}

/**
 * 自动整理状态 + 最近 24 小时整理成功的个数。个数读不到（命令不存在、出错）就带 null：状态照常能用，
 * 界面把「整理了 N 个」整句去掉，不拿 token 除一除去估。状态本身读不到仍然抛错，和 `getAiAutomation` 一样。
 */
export async function getAiAutomationWithRecent(): Promise<AutomationStatus> {
  const [status, queue] = await Promise.all([getAiAutomation(), getAiQueueStatus().catch(() => null)]);
  return withRecentDone(status, queue);
}

export function getGithubAuthState(): Promise<GitHubAuthState> {
  return call<GitHubAuthState>('get_github_auth_state');
}

/** 把后端报错收成 Fox 能看懂的一句话。命令还没合并时，告诉他功能还没接上。 */
/**
 * 浏览器 / 页面代码抛出来的英文技术错误（不是后台写给用户看的中文提示）：翻成一句人话。
 * 认不出来就返回 null，原样显示。
 */
export function plainTechnicalError(raw: string): string | null {
  if (/writeText|clipboard/i.test(raw)) return '复制没成功，请再点一次。';
  if (/Failed to fetch|NetworkError|Load failed|network request failed/i.test(raw)) return '网络没连上，稍后再试一次。';
  if (/Cannot read propert|is not a function|is not an object|is not iterable|undefined is not|null is not|is not defined|Unexpected token|JSON\.parse|not valid JSON/i.test(raw)) {
    return '页面出了点小问题。你的资料都还在。点「重试」；还不行就重启应用，再不行把这条截图发给开发者。';
  }
  return null;
}

export function describeInvokeError(reason: unknown): string {
  const raw = readErrorMessage(reason).trim();
  if (isMissingCommand(raw)) return '这个功能还没接上。';
  const technical = plainTechnicalError(raw);
  if (technical) return technical;
  const cleaned = raw.replace(/^Error:\s*/i, '').trim();
  if (!cleaned) return '操作没有完成，请稍后再试。';
  const chars = Array.from(cleaned);
  if (chars.length > 180) return `${chars.slice(0, 180).join('')}…`;
  return cleaned;
}

/** 后端还没有这个命令（比如前端先发布、后台下次更新才带上）。 */
export function isMissingCommandError(reason: unknown): boolean {
  return isMissingCommand(readErrorMessage(reason));
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
  // 只认「这个命令不存在」。业务上的 404 不是命令缺失。
  return /command\s+\S+\s+not found/i.test(raw)
    || raw.includes('没有这个命令')
    || raw.includes('命令不存在')
    || raw.includes('未知命令');
}

/* ───────── 应用更新 ───────── */

export type UpdaterFailure = 'network' | 'not-found' | 'signature' | 'permission' | 'unknown';

/**
 * 按原因把应用更新的报错分开（更新插件抛出来的是英文原文，只看关键词）。
 * 顺序有讲究：先签名，再网络，最后才看「文件没找到」。网络失败的报错里常常带着请求地址（含 latest.json），
 * 如果先看 latest.json，断网也会被说成「更新文件没找到」。
 */
export function classifyUpdaterError(error: unknown): UpdaterFailure {
  const raw = readErrorMessage(error).toLowerCase();
  if (/signature|pubkey|public key|minisign/.test(raw)) return 'signature';
  if (/network|dns|timed out|timeout|failed to fetch|error sending request|error trying to connect|connection (refused|reset|closed|aborted)|could not resolve|failed to lookup|offline|unreachable|\btls\b|certificate/.test(raw)) {
    return 'network';
  }
  if (/valid release json|latest\.json|\b404\b|not found/.test(raw)) return 'not-found';
  if (/permission|operation not permitted|access denied|read-only/.test(raw)) return 'permission';
  return 'unknown';
}

/** 认不出原因时的兜底：不乱说「文件没找到」，只说这次没成、不影响资料、下一步怎么办。 */
export const UPDATE_FAILED_FALLBACK = '这次没检查成功。不影响你的资料，稍后再试；一直不行就把这条截图发给开发者。';
export const UPDATE_INSTALL_FAILED_FALLBACK = '这次没更新成功。不影响你的资料和现在用的版本，稍后再试；一直不行就把这条截图发给开发者。';

/** 检查更新、或下载安装更新失败：每种原因一句话，写清下一步。 */
export function describeUpdaterError(error: unknown, phase: 'check' | 'install' = 'check'): string {
  switch (classifyUpdaterError(error)) {
    case 'network':
      return '没连上更新服务器。检查一下网络，稍后再试一次。';
    case 'not-found':
      return '更新文件没找到。可能新版本还没发布好，请稍后再试；一直不行，就到项目的 GitHub 页面手动下载新版本。';
    case 'signature':
      return '更新文件校验没通过，没有安装，现在用的版本和你的资料都不受影响。请稍后再试；一直不行，就到项目的 GitHub 页面手动下载新版本。';
    case 'permission':
      return '更新文件没能写进应用所在的位置。请把应用放在「应用程序」文件夹里再试一次；还不行就手动下载新版本。';
    case 'unknown':
    default:
      return phase === 'install' ? UPDATE_INSTALL_FAILED_FALLBACK : UPDATE_FAILED_FALLBACK;
  }
}

/** 更新已经装好，但应用没能自己重新启动：和「更新失败」是两回事，下一步是重启，不是重试更新。 */
export const UPDATE_RELAUNCH_FAILED = '更新已经装好了，但应用没能自己重新启动。请再点一次「重启应用」，或者手动退出后重新打开。';

/** 更新已经装好，也发出了重启请求，但窗口一直没关。 */
export const UPDATE_WINDOW_STILL_OPEN = '更新已安装，但当前窗口仍未关闭。请再次点击「重启应用」，或手动退出后重新打开。';

export type UpdateStatusInput = {
  status: string;
  availableVersion: string | null;
  currentVersion: string;
  errorMessage: string | null;
};

/** 「应用更新」面板里状态那一句。当前版本还没读到时，不说「已是最新版本」：不知道版本，就不能下这个结论。 */
export function describeUpdateStatus(input: UpdateStatusInput): string {
  switch (input.status) {
    case 'checking':
      return '正在检查更新…';
    case 'available':
      return `发现新版本 ${input.availableVersion ?? ''}，可查看说明后立即安装。`;
    case 'not-available':
      return input.currentVersion ? '已是最新版本。' : '还没读到当前版本，先不判断是不是最新。';
    case 'downloading':
      return '正在下载并安装更新，请保持应用打开。';
    case 'installed':
      return input.errorMessage ?? '更新已安装，正在准备重启应用。';
    case 'relaunching':
      return '更新已安装，正在重启应用。';
    case 'error':
      return input.errorMessage ?? UPDATE_FAILED_FALLBACK;
    case 'idle':
    default:
      return '应用会在启动时静默检查更新，也可以在这里手动检查。';
  }
}
