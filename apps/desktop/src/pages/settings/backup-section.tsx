import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { downloadDir, join } from '@tauri-apps/api/path';
import { relaunch } from '@tauri-apps/plugin-process';
import type { useWorkspace } from '@/providers/workspace-provider';
import { Button, useToast } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import {
  backupKindLabel,
  backupTimeLine,
  EXPORT_SECTION_HINT,
  exportSavedText,
  formatMinute,
  formatSize,
  GIST_BACKUP_LIBRARY,
  GIST_BACKUP_MARKS,
  GIST_IMPORT_WARNING,
  GIST_NEEDS_SCOPE,
  GIST_SECTION_TITLE,
  gistImportHint,
} from '@/lib/automation-view';
import { errorText, Notice, Section } from './shared';

type BackupKind = 'auto' | 'manual' | 'preRestore' | 'preMigrate';
type BackupInfo = {
  fileName: string;
  kind: BackupKind;
  createdAt: string;
  bytes: number;
  repositoryCount: number | null;
  schemaVersion: string | null;
};
type RestoreReport = {
  ok: boolean;
  fileName: string;
  backupCreatedAt: string | null;
  preRestoreFileName: string | null;
  error: string | null;
};
type BackupStatus = {
  lastAutoAt: string | null;
  nextAutoAt: string;
  lastRestore: RestoreReport | null;
};

function restoreBanner(report: RestoreReport): string {
  if (report.ok) {
    const when = report.backupCreatedAt ? formatMinute(report.backupCreatedAt) : report.fileName;
    return `已恢复到 ${when} 的备份`;
  }
  return `恢复失败：${report.error ?? '未知原因'}（资料库没有被改动）`;
}

export function BackupSection({ workspace }: { workspace: ReturnType<typeof useWorkspace> }) {
  const toast = useToast();
  const [status, setStatus] = useState<BackupStatus | null>(null);
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [confirmName, setConfirmName] = useState<string | null>(null);
  const [restartReady, setRestartReady] = useState(false);
  const [backingUp, setBackingUp] = useState(false);

  const isConnected = workspace.authState.hasToken && Boolean(workspace.authState.user);
  const isAnyGistActionRunning =
    workspace.isExportingAnnotations ||
    workspace.isImportingAnnotations ||
    workspace.isExportingRepositoryLibrary ||
    workspace.isImportingRepositoryLibrary;
  const importHint = gistImportHint(isConnected, Boolean(workspace.gistIdDraft.trim()));

  function say(title: string) {
    toast.show({ title });
  }

  useEffect(() => {
    invoke<BackupStatus>('get_backup_status').then(setStatus).catch((reason: unknown) => say(errorText(reason)));
    invoke<BackupInfo[]>('list_backups').then(setBackups).catch((reason: unknown) => say(errorText(reason)));
  }, []);

  function backupNow() {
    setBackingUp(true);
    void invoke<BackupInfo>('create_backup_now')
      .then(() => {
        say('已备份');
        return invoke<BackupInfo[]>('list_backups').then(setBackups);
      })
      .then(() => invoke<BackupStatus>('get_backup_status').then(setStatus))
      .catch((reason: unknown) => say(errorText(reason)))
      .finally(() => setBackingUp(false));
  }

  function confirmRestore() {
    if (!confirmName) return;
    const fileName = confirmName;
    void invoke('request_restore_backup', { fileName })
      .then(() => {
        setConfirmName(null);
        setRestartReady(true);
        say('已记录，重启后生效');
      })
      .catch((reason: unknown) => say(errorText(reason)));
  }

  async function exportLibrary(command: 'export_library_markdown' | 'export_library_csv', fileName: string) {
    try {
      const destination = await join(await downloadDir(), fileName);
      const count = await invoke<number>(command, { destination });
      say(exportSavedText(count, fileName));
    } catch (reason: unknown) {
      say(errorText(reason));
    }
  }

  return (
    <div className="space-y-6">
      {status?.lastRestore ? <Notice>{restoreBanner(status.lastRestore)}</Notice> : null}
      {restartReady ? (
        <Notice>
          <span>已记录，重启应用后恢复生效。</span>
          <Button onClick={() => void relaunch().catch((reason: unknown) => say(errorText(reason)))}>立即重启</Button>
          <Button variant="text" onClick={() => setRestartReady(false)}>稍后</Button>
        </Notice>
      ) : null}

      <Section
        icon="backup"
        hue={148}
        title="本机备份（自动，存在这台电脑）"
        hint={status ? backupTimeLine(status.lastAutoAt) : '读取中'}
        actions={<Button disabled={backingUp} loading={backingUp} onClick={backupNow}>现在备份</Button>}
      >
        {confirmName ? (
          <Notice tone="warn">
            <span className="min-w-0 flex-1">恢复会把当前资料库换成这一份。当前的资料库会先另存一份「恢复前自动留的」备份。需要重启应用才会生效。继续吗？</span>
            <Button onClick={confirmRestore}>继续恢复</Button>
            <Button variant="text" onClick={() => setConfirmName(null)}>取消</Button>
          </Notice>
        ) : null}
        <ul className="m-0 flex list-none flex-col gap-1.5 p-0">
          {backups.slice(0, 6).map((item) => (
            <li key={item.fileName} className="flex items-center gap-3 rounded-lg border border-border px-3 py-2 text-sm">
              <span className="min-w-0 flex-1 truncate text-foreground">
                {formatMinute(item.createdAt)}
                <span className="ml-2 text-xs text-on-surface-variant">
                  {backupKindLabel(item.kind)} · {formatSize(item.bytes)} · {item.repositoryCount === null ? '工具数未知' : `${item.repositoryCount} 个工具`}
                </span>
              </span>
              <Button size="sm" onClick={() => setConfirmName(item.fileName)}>恢复到这一份…</Button>
            </li>
          ))}
        </ul>
        {backups.length > 6 ? (
          <p className="mt-2 mb-0 text-xs text-subtle">只显示最近 6 份，更早的在「外观与关于 → 数据文件夹」的 backups 目录里。</p>
        ) : null}
      </Section>

      <Section
        icon="ios_share"
        hue={28}
        title="导出成文档 / 表格"
        hint={EXPORT_SECTION_HINT}
      >
        <div className="flex flex-wrap gap-2">
          <Button onClick={() => void exportLibrary('export_library_markdown', 'fox-stars-lab-library.md')}>导出成文档（Markdown）</Button>
          <Button onClick={() => void exportLibrary('export_library_csv', 'fox-stars-lab-library.csv')}>导出成表格（CSV，可用 Excel 打开）</Button>
        </div>
      </Section>

      <section className="glass-panel rounded-xl p-6">
        <h3 className="font-headline-md mb-6 text-[20px] font-semibold text-on-surface">{GIST_SECTION_TITLE}</h3>
        <div className="space-y-6">
          {!isConnected && (
            <p className="font-body-md rounded-lg border border-error/20 bg-error/10 px-3 py-2 text-sm text-error">
              {GIST_NEEDS_SCOPE}
            </p>
          )}
          {workspace.authMessage && (
            <p className="font-body-md rounded-lg border border-success/20 bg-success/10 px-3 py-2 text-sm text-success">
              {workspace.authMessage}
            </p>
          )}
          {workspace.error && (
            <p className="font-body-md rounded-lg border border-error/20 bg-error/10 px-3 py-2 text-sm text-error">
              {workspace.error}
            </p>
          )}
          <div className="rounded-lg border border-card-border bg-surface-container-low p-4">
            <h4 className="font-body-lg mb-2 flex items-center gap-2 font-medium text-on-surface">
              <Icon name="cloud_upload" size={20} className="text-primary" />
              {GIST_BACKUP_MARKS.title}
            </h4>
            <p className="font-body-md mb-3 text-sm text-on-surface-variant">{GIST_BACKUP_MARKS.body}</p>
            <button
              type="button"
              onClick={() => void workspace.handleExportAnnotations()}
              disabled={isAnyGistActionRunning || !isConnected}
              title={isConnected ? '把标记、标签和笔记上传到你的私密 Gist' : '请先连接 GitHub 账号'}
              className="font-body-md flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-white shadow-sm transition-all hover:brightness-110 disabled:opacity-60"
            >
              <Icon name="upload" size={18} />
              {workspace.isExportingAnnotations ? '上传中…' : GIST_BACKUP_MARKS.title}
            </button>
          </div>
          <div className="rounded-lg border border-card-border bg-surface-container-low p-4">
            <h4 className="font-body-lg mb-2 flex items-center gap-2 font-medium text-on-surface">
              <Icon name="inventory_2" size={20} className="text-primary" />
              {GIST_BACKUP_LIBRARY.title}
            </h4>
            <p className="font-body-md mb-3 text-sm text-on-surface-variant">{GIST_BACKUP_LIBRARY.body}</p>
            <button
              type="button"
              onClick={() => void workspace.handleExportRepositoryLibrary()}
              disabled={isAnyGistActionRunning || !isConnected}
              title={isConnected ? '把整个清单（含标记、标签和笔记）上传到你的私密 Gist' : '请先连接 GitHub 账号'}
              className="font-body-md flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-white shadow-sm transition-all hover:brightness-110 disabled:opacity-60"
            >
              <Icon name="upload" size={18} />
              {workspace.isExportingRepositoryLibrary ? '上传中…' : GIST_BACKUP_LIBRARY.title}
            </button>
          </div>
          <div className="rounded-lg border border-card-border bg-surface-container-low p-4">
            <h4 className="font-body-lg mb-2 flex items-center gap-2 font-medium text-on-surface">
              <Icon name="cloud_download" size={20} className="text-primary" />
              从 GitHub 导入备份
            </h4>
            <p className="font-body-md mb-3 text-sm text-on-surface-variant">
              填入上传备份时得到的 Gist ID（备份的编号），再选要导入什么。导入整个清单时，这台电脑上还没有的收藏会补上。
            </p>
            <p className="font-body-md mb-3 rounded-lg border border-warning/25 bg-warning/10 px-3 py-2 text-xs text-warning">
              {GIST_IMPORT_WARNING}
            </p>
            <form onSubmit={(event) => void workspace.handleImportAnnotations(event)} className="space-y-3">
              <input
                type="text"
                value={workspace.gistIdDraft}
                onChange={(e) => workspace.setGistIdDraft(e.target.value)}
                placeholder="粘贴 Gist ID"
                className="font-body-md w-full rounded-lg border border-outline-variant bg-surface px-4 py-2 text-on-surface focus:border-primary focus:ring-1 focus:ring-primary focus:outline-none"
              />
              <div className="flex flex-col gap-2 sm:flex-row sm:gap-3">
                <button
                  type="submit"
                  disabled={isAnyGistActionRunning || !workspace.gistIdDraft.trim() || !isConnected}
                  title={isConnected ? '从 Gist 恢复这台电脑上已有工具的标记、标签和笔记' : '请先连接 GitHub 账号'}
                  className="font-body-md rounded-lg border border-card-border bg-surface-container-high px-4 py-2 text-on-surface transition-all hover:bg-surface-container-highest disabled:opacity-60"
                >
                  {workspace.isImportingAnnotations ? '导入中…' : '导入我的标记'}
                </button>
                <button
                  type="button"
                  onClick={(event) => void workspace.handleImportRepositoryLibrary(event)}
                  disabled={isAnyGistActionRunning || !workspace.gistIdDraft.trim() || !isConnected}
                  title={isConnected ? '从 Gist 恢复整个清单（含标记、标签和笔记）' : '请先连接 GitHub 账号'}
                  className="font-body-md rounded-lg border border-card-border bg-surface-container-high px-4 py-2 text-on-surface transition-all hover:bg-surface-container-highest disabled:opacity-60"
                >
                  {workspace.isImportingRepositoryLibrary ? '导入中…' : '导入整个清单'}
                </button>
              </div>
            </form>
            {importHint && (
              <p className={`font-body-md mt-2 text-xs ${isConnected ? 'text-on-surface-variant' : 'text-error'}`}>
                {importHint}
              </p>
            )}
          </div>
        </div>
      </section>
    </div>
  );
}
