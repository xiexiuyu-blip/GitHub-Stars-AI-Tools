import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { useAppSettings } from '@/providers/settings-provider';
import type { useWorkspace } from '@/providers/workspace-provider';
import type { AppUpdateContextValue } from '@/providers/app-update-provider';
import { AppUpdatePanel } from '@/components/app-update-panel';
import { Button, Kbd, useToast } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import {
  CLEAR_LOCAL_DATA_DONE,
  CLEAR_LOCAL_DATA_FAILED,
  CLEAR_LOCAL_DATA_WARNING,
  runStatusText,
  WELCOME_STARTUP_HINT,
  type AutomationStatus,
} from '@/lib/automation-view';
import { getAiAutomationWithRecent } from '@/lib/api';
import { COLOR_PRESETS, type ThemeSettings as ThemeSettingsValue } from '@/types-settings';
import type { AppIdentity } from '@/types';
import { errorText, Section, ToggleSwitch, toErrorMessage } from './shared';

type Job = { id: string; jobType: string; status: string; errorKind: string | null };

const PROJECT_REPOSITORY_URL = 'https://github.com/xiexiuyu-blip/GitHub-Stars-AI-Tools/tree/fox-product-lab';
const PROJECT_ISSUES_URL = 'https://github.com/xiexiuyu-blip/GitHub-Stars-AI-Tools/issues';
const PROJECT_LICENSE_URL = 'https://github.com/xiexiuyu-blip/GitHub-Stars-AI-Tools/blob/fox-product-lab/LICENSE';
const PROJECT_ACKNOWLEDGEMENTS_URL = 'https://github.com/xiexiuyu-blip/GitHub-Stars-AI-Tools/tree/fox-product-lab#%E8%87%B4%E8%B0%A2';

const SHORTCUT_ITEMS: Array<{ keys: string[]; label: string }> = [
  { keys: ['⌘K'], label: '打开搜索与快捷命令面板' },
  { keys: ['/'], label: '在「资料库」聚焦搜索框' },
  { keys: ['J / K', '↑ / ↓'], label: '在「今日」和「资料库」列表上下切换项目' },
  { keys: ['T'], label: '把当前项目标为「想试」' },
  { keys: ['U'], label: '把当前项目设为「没看」' },
  { keys: ['X'], label: '把当前项目标为「不要了」' },
  { keys: ['C'], label: '复制当前项目交接给 AI 助手' },
];

export function GeneralAboutSection({
  settingsHook,
  workspace,
  appUpdate,
  onOpenBackup,
}: {
  settingsHook: ReturnType<typeof useAppSettings>;
  workspace: ReturnType<typeof useWorkspace>;
  appUpdate: AppUpdateContextValue;
  onOpenBackup: () => void;
}) {
  const toast = useToast();
  const [isClearingAllLocalData, setIsClearingAllLocalData] = useState(false);
  const [isClearDataConfirmOpen, setIsClearDataConfirmOpen] = useState(false);
  const [localDataMessage, setLocalDataMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
  const [appIdentity, setAppIdentity] = useState<AppIdentity | null>(null);
  const [appIdentityError, setAppIdentityError] = useState<string | null>(null);
  const [health, setHealth] = useState('');
  const [jobs, setJobs] = useState<Job[]>([]);
  const [automation, setAutomation] = useState<AutomationStatus | null>(null);

  function loadDiagnostics() {
    invoke<string>('get_health_summary').then(setHealth).catch((reason: unknown) => setHealth(String(reason)));
    invoke<Job[]>('list_ai_jobs').then(setJobs).catch(() => setJobs([]));
    getAiAutomationWithRecent().then(setAutomation).catch(() => undefined);
  }

  useEffect(() => {
    let cancelled = false;
    invoke<AppIdentity>('get_app_identity')
      .then((identity) => {
        if (!cancelled) {
          setAppIdentity(identity);
          setAppIdentityError(null);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setAppIdentity(null);
          setAppIdentityError(toErrorMessage(error));
        }
      });
    loadDiagnostics();
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleClearAllLocalData() {
    setIsClearingAllLocalData(true);
    setLocalDataMessage(null);
    try {
      await settingsHook.resetSettings();
      await workspace.handleClearLocalData();
      setIsClearDataConfirmOpen(false);
      setLocalDataMessage({ type: 'success', text: CLEAR_LOCAL_DATA_DONE });
    } catch {
      setLocalDataMessage({ type: 'error', text: CLEAR_LOCAL_DATA_FAILED });
    } finally {
      setIsClearingAllLocalData(false);
    }
  }

  return (
    <div className="space-y-6">
      <section className="glass-panel rounded-xl p-6">
        <h3 className="font-headline-md mb-6 text-[20px] font-semibold text-on-surface">外观与通用</h3>
        <div className="space-y-6">
          <div className="flex flex-col gap-3 py-2 md:flex-row md:items-center md:justify-between">
            <div className="min-w-0 flex-1 md:pr-6">
              <h4 className="font-body-lg font-medium text-on-surface">界面主题</h4>
              <p className="font-body-md mt-1 text-on-surface-variant">选择浅色、深色，或跟随系统外观自动切换。</p>
            </div>
            <div className="flex w-full rounded-lg border border-outline-variant/30 bg-surface-container-low p-1 md:w-auto">
              {[
                { value: 'system', label: '系统' },
                { value: 'light', label: '浅色' },
                { value: 'dark', label: '深色' },
              ].map((item) => (
                <button
                  key={item.value}
                  type="button"
                  onClick={() => void settingsHook.updateTheme({ mode: item.value as ThemeSettingsValue['mode'] })}
                  className={`flex-1 rounded-md px-3 py-1.5 text-sm transition-colors md:flex-none ${
                    settingsHook.settings.theme.mode === item.value
                      ? 'bg-primary text-white'
                      : 'text-on-surface-variant hover:text-on-surface'
                  }`}
                >
                  {item.label}
                </button>
              ))}
            </div>
          </div>

          <div>
            <label className="font-body-md mb-2 block font-medium text-on-surface">品牌色</label>
            <div className="flex gap-2">
              {COLOR_PRESETS.map((preset) => (
                <button
                  key={preset.name}
                  type="button"
                  title={preset.label}
                  onClick={() => void settingsHook.updateTheme({ brandColor: preset.color, colorPreset: preset.name })}
                  className={`h-10 w-10 rounded-full border-2 transition-transform hover:scale-110 ${
                    settingsHook.settings.theme.brandColor === preset.color
                      ? 'scale-110 border-on-surface'
                      : 'border-transparent'
                  }`}
                  style={{ backgroundColor: preset.color }}
                />
              ))}
            </div>
          </div>

          <div className="flex flex-col gap-3 border-t border-card-border py-4 md:flex-row md:items-center md:justify-between">
            <div className="min-w-0 flex-1 md:pr-6">
              <h4 className="font-body-lg font-medium text-on-surface">界面字号</h4>
              <p className="font-body-md mt-1 text-on-surface-variant">调整主要文字、标题和标签的显示尺寸。</p>
            </div>
            <div className="flex w-full rounded-lg border border-outline-variant/30 bg-surface-container-low p-1 md:w-auto">
              {[
                { value: 'small', label: '小' },
                { value: 'medium', label: '中' },
                { value: 'large', label: '大' },
              ].map((item) => (
                <button
                  key={item.value}
                  type="button"
                  onClick={() => void settingsHook.updateTheme({ fontSize: item.value as ThemeSettingsValue['fontSize'] })}
                  className={`flex-1 rounded-md px-3 py-1.5 text-sm transition-colors md:flex-none ${
                    settingsHook.settings.theme.fontSize === item.value
                      ? 'bg-primary text-white'
                      : 'text-on-surface-variant hover:text-on-surface'
                  }`}
                >
                  {item.label}
                </button>
              ))}
            </div>
          </div>

          <div className="flex items-start justify-between gap-4 border-t border-card-border py-4">
            <div className="min-w-0 flex-1">
              <h4 className="font-body-lg font-medium text-on-surface">启动时显示欢迎引导</h4>
              <p className="font-body-md mt-1 text-on-surface-variant">{WELCOME_STARTUP_HINT}</p>
            </div>
            <ToggleSwitch
              checked={settingsHook.settings.general.showWelcomeOnStartup}
              onChange={(checked) => void settingsHook.updateGeneral({ showWelcomeOnStartup: checked })}
            />
          </div>
        </div>
      </section>

      <Section icon="keyboard" hue={222} title="快捷键" hint="在桌面界面随时可用的键盘操作。">
        <ul className="m-0 grid list-none gap-2 p-0 sm:grid-cols-2">
          {SHORTCUT_ITEMS.map((item) => (
            <li key={item.label} className="flex items-center justify-between gap-3 rounded-lg border border-border px-3 py-2 text-sm">
              <span className="text-on-surface-variant">{item.label}</span>
              <span className="flex shrink-0 items-center gap-1">
                {item.keys.map((key) => <Kbd key={key}>{key}</Kbd>)}
              </span>
            </li>
          ))}
        </ul>
      </Section>

      <section className="glass-panel rounded-xl p-6">
        <h3 className="font-headline-md mb-6 text-[20px] font-semibold text-on-surface">关于</h3>
        <div className="space-y-5">
          <div className="rounded-xl border border-outline-variant/30 bg-surface-container-low p-4">
            <div className="flex items-center gap-2">
              <Icon name="stars" size={20} className="shrink-0 text-primary" />
              <h4 className="font-body-lg min-w-0 font-semibold text-on-surface">Fox Stars Lab</h4>
            </div>
            <dl className="mt-4 grid gap-x-4 gap-y-3 text-sm md:grid-cols-2">
              <IdentityField label="版本号" value={appUpdate.currentVersion || '读取中'} />
              <IdentityField
                label="数据放在哪个文件夹"
                value={appIdentity?.dataDir ?? (appIdentityError ? '没读出来。你的资料都还在。' : '正在读取…')}
              />
              <div className="min-w-0 border-b border-outline-variant/20 pb-2 md:col-span-2">
                <dt className="text-xs font-medium text-on-surface-variant">许可证</dt>
                <dd className="mt-1 text-sm leading-relaxed text-on-surface">
                  非商用许可：个人学习、研究和非营利用途可以使用；商业使用、集成或再分发需要另行获得授权。
                  <a href={PROJECT_LICENSE_URL} target="_blank" rel="noreferrer" className="ml-2 text-primary hover:underline">
                    查看许可证全文
                  </a>
                </dd>
              </div>
            </dl>
          </div>
          <AppUpdatePanel appUpdate={appUpdate} />
        </div>
      </section>

      <Section
        icon="monitor_heart"
        hue={220}
        title="运行状态与诊断"
        hint={`${runStatusText(automation?.failed)} · 右侧按钮只复制应用状态与最近 8 条整理任务进展，不含口令和密钥，可直接发给开发者。`}
        actions={
          <Button
            onClick={() => {
              const lines = [
                health,
                ...jobs.slice(0, 8).map((job) => `${job.jobType} · ${job.status}${job.errorKind ? ` · ${job.errorKind}` : ''}`),
              ];
              void navigator.clipboard.writeText(lines.filter(Boolean).join('\n'))
                .then(() => toast.show({ title: '诊断信息已复制' }))
                .catch((reason: unknown) => toast.show({ title: errorText(reason) }));
            }}
          >
            复制诊断信息
          </Button>
        }
      >
        <details className="group">
          <summary className="flex cursor-pointer list-none items-center gap-1 text-xs text-on-surface-variant [&::-webkit-details-marker]:hidden">
            <Icon name="chevron_right" size={16} className="transition-transform group-open:rotate-90" />
            开发者与运行明细
          </summary>
          <div className="mt-3 space-y-4">
            <p className="m-0 text-sm leading-6 text-foreground">{health}</p>
            <p className="m-0 max-w-2xl text-xs leading-[1.7] text-on-surface-variant">
              连上的外部程序只能看、不能改你的资料；要写入得单独再开一个入口，默认都是关的。许可证全称：PolyForm Noncommercial 1.0.0。
            </p>
            {jobs.length > 0 ? (
              <ul className="m-0 flex list-none flex-col gap-1.5 p-0 text-sm">
                {jobs.slice(0, 8).map((job) => (
                  <li key={job.id} className="flex items-center justify-between rounded-lg border border-border px-3 py-1.5">
                    <span className="text-on-surface-variant">{job.jobType} · {job.status} {job.errorKind ? `· ${job.errorKind}` : ''}</span>
                    <Button size="sm" variant="text" onClick={() => void invoke('cancel_ai_job', { id: job.id }).then(loadDiagnostics)}>取消</Button>
                  </li>
                ))}
              </ul>
            ) : null}
            <div className="flex flex-wrap gap-2 text-sm">
              <a href={PROJECT_REPOSITORY_URL} target="_blank" rel="noreferrer" className="inline-flex items-center gap-1.5 rounded-lg border border-outline-variant/35 bg-surface px-3 py-1.5 text-on-surface transition-colors hover:bg-surface-container-high">
                <Icon name="code" size={16} />项目的 GitHub 页面<Icon name="open_in_new" size={14} />
              </a>
              <a href={PROJECT_ISSUES_URL} target="_blank" rel="noreferrer" className="inline-flex items-center gap-1.5 rounded-lg border border-outline-variant/35 bg-surface px-3 py-1.5 text-on-surface transition-colors hover:bg-surface-container-high">
                <Icon name="bug_report" size={16} />问题反馈<Icon name="open_in_new" size={14} />
              </a>
              <a href={PROJECT_ACKNOWLEDGEMENTS_URL} target="_blank" rel="noreferrer" className="inline-flex items-center gap-1.5 rounded-lg border border-outline-variant/35 bg-surface px-3 py-1.5 text-on-surface transition-colors hover:bg-surface-container-high">
                <Icon name="diversity_3" size={16} />开源组件鸣谢<Icon name="open_in_new" size={14} />
              </a>
            </div>
            <AppIdentityDetails identity={appIdentity} errorMessage={appIdentityError} />
          </div>
        </details>
      </Section>

      <section className="rounded-xl border border-error/25 bg-error/10 p-5">
        <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <Icon name="delete" size={20} className="text-error" />
              <h4 className="font-body-lg font-semibold text-on-surface">清空本机数据</h4>
            </div>
            <p className="font-body-md mt-2 max-w-2xl text-sm leading-relaxed text-on-surface-variant">
              {CLEAR_LOCAL_DATA_WARNING}
            </p>
            {localDataMessage && (
              <p className={`mt-3 text-sm ${localDataMessage.type === 'success' ? 'text-success' : 'text-error'}`}>
                {localDataMessage.text}
              </p>
            )}
          </div>
          <div className="flex shrink-0 flex-wrap items-center gap-2">
            <Button onClick={onOpenBackup}>先去备份</Button>
            <button
              type="button"
              onClick={() => {
                setLocalDataMessage(null);
                setIsClearDataConfirmOpen(true);
              }}
              disabled={isClearingAllLocalData || workspace.isClearingLocalData}
              className="inline-flex items-center justify-center gap-2 rounded-lg border border-error/30 bg-error px-4 py-2 text-sm font-semibold text-on-error transition-colors hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
            >
              <Icon name="delete" size={17} />
              {isClearingAllLocalData || workspace.isClearingLocalData ? '正在清空' : '清空本机数据'}
            </button>
          </div>
        </div>
        {isClearDataConfirmOpen && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 px-4 py-6 backdrop-blur-sm">
            <div
              role="dialog"
              aria-modal="true"
              aria-labelledby="clear-local-data-title"
              className="w-full max-w-md rounded-lg border border-error/25 bg-surface p-5 shadow-xl shadow-black/20"
            >
              <div className="flex items-start gap-3">
                <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-error/10">
                  <Icon name="delete" size={20} className="text-error" />
                </span>
                <div className="min-w-0">
                  <h4 id="clear-local-data-title" className="font-body-lg font-semibold text-on-surface">确认清空本机数据</h4>
                  <p className="mt-2 text-sm leading-relaxed text-on-surface-variant">{CLEAR_LOCAL_DATA_WARNING}</p>
                </div>
              </div>
              <div className="mt-5 flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
                <button
                  type="button"
                  onClick={() => setIsClearDataConfirmOpen(false)}
                  disabled={isClearingAllLocalData || workspace.isClearingLocalData}
                  className="inline-flex items-center justify-center rounded-lg border border-outline-variant/40 bg-surface-container-low px-4 py-2 text-sm font-medium text-on-surface transition-colors hover:bg-surface-container-high disabled:cursor-not-allowed disabled:opacity-60"
                >
                  取消
                </button>
                <button
                  type="button"
                  onClick={() => void handleClearAllLocalData()}
                  disabled={isClearingAllLocalData || workspace.isClearingLocalData}
                  className="inline-flex items-center justify-center gap-2 rounded-lg bg-error px-4 py-2 text-sm font-semibold text-on-error transition-colors hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  <Icon name="delete" size={17} />
                  {isClearingAllLocalData || workspace.isClearingLocalData ? '正在清空' : '确认清空'}
                </button>
              </div>
            </div>
          </div>
        )}
      </section>
    </div>
  );
}

function AppIdentityDetails({ identity, errorMessage }: { identity: AppIdentity | null; errorMessage: string | null }) {
  const identityRows = identity ? [
    { label: '显示名称', value: identity.displayName },
    { label: 'Bundle ID', value: identity.bundleIdentifier },
    { label: 'App Bundle', value: identity.appBundleName },
    { label: '安装包命名', value: identity.dmgFilePattern },
    { label: '配置文件夹', value: identity.configDir },
    { label: '资料库文件', value: identity.databasePath },
    { label: '设置文件', value: identity.settingsPath },
    { label: '钥匙串服务名', value: identity.credentialService },
    { label: 'GitHub 备份文件名', value: identity.annotationGistFile },
    { label: '更新地址', value: identity.updateEndpoint },
    { label: '项目的 GitHub 页面', value: identity.repositoryUrl },
  ] : [];

  return (
    <div className="border-t border-outline-variant/25 pt-4">
      <div className="mb-3 flex items-center gap-2">
        <Icon name="fingerprint" size={19} className="text-primary" />
        <h5 className="font-body-md font-semibold text-on-surface">应用身份与本机位置</h5>
      </div>
      {errorMessage ? (
        <p className="rounded-lg border border-error/20 bg-error/10 px-3 py-2 text-sm text-error">
          没读出来。你的资料都还在。重启应用再试；还不行就把这条截图发给开发者。
        </p>
      ) : null}
      {!identity && !errorMessage ? (
        <p className="rounded-lg border border-outline-variant/25 bg-surface px-3 py-2 text-sm text-on-surface-variant">
          正在读取…
        </p>
      ) : null}
      {identity ? (
        <dl className="grid gap-x-4 gap-y-3 text-sm md:grid-cols-2">
          {identityRows.map((row) => <IdentityField key={row.label} label={row.label} value={row.value} />)}
        </dl>
      ) : null}
    </div>
  );
}

function IdentityField({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 border-b border-outline-variant/20 pb-2">
      <dt className="text-xs font-medium text-on-surface-variant">{label}</dt>
      <dd className="mt-1 break-words font-mono text-[12px] leading-5 text-on-surface [overflow-wrap:anywhere]" title={value}>
        {value}
      </dd>
    </div>
  );
}
