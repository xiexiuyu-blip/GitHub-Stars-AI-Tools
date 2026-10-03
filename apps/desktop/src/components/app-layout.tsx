import { FormEvent, ReactNode, useEffect, useRef, useState } from 'react';
import { BrandIcon } from '@/components/ui/brand-icon';
import { Icon } from '@/components/ui/icon';
import { NAV_ITEMS, SETTINGS_ITEM, navKeyFor, type NavItem } from '@/lib/nav';
import type { AppPage, GitHubUser, StarSyncSummary, TaskProgressEvent } from '@/types';

type AppLayoutProps = {
  children: ReactNode;
  currentPage: AppPage;
  onNavigate: (page: AppPage) => void;
  user: GitHubUser | null;
  onSyncStars: () => void;
  isSyncing: boolean;
  onFetchReadmes: () => void;
  isFetchingReadmes: boolean;
  onBatchGenerateAiDocuments: () => void;
  isBatchGeneratingAiDocuments: boolean;
  onGenerateAiTagNetwork: () => void;
  isGeneratingTagNetwork: boolean;
  onCheckForUpdate: () => void;
  isCheckingUpdate: boolean;
  syncSummary: StarSyncSummary | null;
  onGlobalSearch: (query: string) => void;
  taskProgress: TaskProgressEvent | null;
  onRetryTask: (() => void) | null;
  retryTaskLabel: string | null;
  isRetryingTask: boolean;
  statusMessage: string | null;
  errorMessage: string | null;
  notificationOpenSignal: number;
  onOpenCommandPalette: () => void;
};

export function AppLayout(props: AppLayoutProps) {
  const [searchQuery, setSearchQuery] = useState('');
  const [isNotificationOpen, setIsNotificationOpen] = useState(false);
  const [isQuickActionsOpen, setIsQuickActionsOpen] = useState(false);
  const topbarActionsRef = useRef<HTMLDivElement | null>(null);
  const activeNav = navKeyFor(props.currentPage);

  useEffect(() => {
    if (props.notificationOpenSignal <= 0) {
      return;
    }

    setIsNotificationOpen(true);
    setIsQuickActionsOpen(false);
  }, [props.notificationOpenSignal]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        props.onOpenCommandPalette();
      }
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [props.onOpenCommandPalette]);

  useEffect(() => {
    if (!isQuickActionsOpen && !isNotificationOpen) {
      return undefined;
    }

    function handlePointerDown(event: PointerEvent) {
      const target = event.target;
      if (!(target instanceof Node)) {
        return;
      }

      if (topbarActionsRef.current?.contains(target)) {
        return;
      }

      setIsQuickActionsOpen(false);
      setIsNotificationOpen(false);
    }

    document.addEventListener('pointerdown', handlePointerDown);
    return () => document.removeEventListener('pointerdown', handlePointerDown);
  }, [isQuickActionsOpen, isNotificationOpen]);

  function handleSearchSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const query = searchQuery.trim();
    if (!query) {
      return;
    }
    setIsQuickActionsOpen(false);
    props.onGlobalSearch(query);
  }

  const syncText = props.isSyncing
    ? '正在同步…'
    : props.user
      ? (props.syncSummary ? '已同步' : '尚未同步')
      : '未连接';
  // 绿点只给「真的同步过」：连着账号但还没同步是灰点
  const syncDotState = props.isSyncing ? 'syncing' : props.user && props.syncSummary ? 'ok' : 'off';

  return (
    <div className="app-layout flex h-full min-h-0 min-w-0 flex-col overflow-hidden">
      <header className="app-topbar">
        <div className="app-brand">
          <BrandIcon
            tone="brand"
            title="Fox Stars Lab"
            className="app-brand-icon-compact size-[22px]"
          />
          <h1 className="app-brand-name" title="Fox Stars Lab">Fox Stars Lab</h1>
        </div>
        <button
          type="button"
          className="app-search"
          onClick={props.onOpenCommandPalette}
          title="打开命令面板（⌘K）"
          aria-label="打开命令面板"
          aria-keyshortcuts="Meta+K"
        >
          <Icon name="search" size={16} />
          <span className="app-search-label">搜索仓库、标签或内容</span>
          <kbd>⌘K</kbd>
        </button>
        <div ref={topbarActionsRef} className="app-top-actions">
          <span className="app-sync">
            <span className="app-sync-dot" data-state={syncDotState} aria-hidden="true" />
            <span className="app-sync-text">{syncText}</span>
          </span>
          <button
            type="button"
            className="app-sync-button"
            onClick={() => (props.user ? props.onSyncStars() : props.onNavigate('settings'))}
            disabled={props.isSyncing}
            title={props.user ? '同步 GitHub Stars' : '请先连接 GitHub 账号'}
          >
            {props.isSyncing ? '同步中' : '同步'}
          </button>
          <button
            type="button"
            className="app-icon-button"
            onClick={() => {
              setIsQuickActionsOpen((current) => !current);
              setIsNotificationOpen(false);
            }}
            title="快速操作"
            aria-label="快速操作"
            aria-expanded={isQuickActionsOpen}
          >
            <Icon name="bolt" size={18} />
          </button>
          {isQuickActionsOpen && (
            <TopbarQuickActionsMenu
              hasGitHubUser={Boolean(props.user)}
              isSyncing={props.isSyncing}
              onSyncStars={props.onSyncStars}
              onOpenSettings={() => {
                setIsQuickActionsOpen(false);
                props.onNavigate('settings');
              }}
              onFetchReadmes={() => {
                setIsQuickActionsOpen(false);
                props.onFetchReadmes();
              }}
              isFetchingReadmes={props.isFetchingReadmes}
              onBatchGenerateAiDocuments={() => {
                setIsQuickActionsOpen(false);
                props.onBatchGenerateAiDocuments();
              }}
              isBatchGeneratingAiDocuments={props.isBatchGeneratingAiDocuments}
              onGenerateAiTagNetwork={() => {
                setIsQuickActionsOpen(false);
                props.onGenerateAiTagNetwork();
              }}
              isGeneratingTagNetwork={props.isGeneratingTagNetwork}
              onCheckForUpdate={() => {
                setIsQuickActionsOpen(false);
                props.onCheckForUpdate();
              }}
              isCheckingUpdate={props.isCheckingUpdate}
              searchQuery={searchQuery}
              onSearchQueryChange={setSearchQuery}
              onGlobalSearchSubmit={handleSearchSubmit}
            />
          )}
          <button
            type="button"
            className="app-icon-button"
            onClick={() => {
              setIsNotificationOpen((current) => !current);
              setIsQuickActionsOpen(false);
            }}
            title="查看通知和任务状态"
            aria-label="查看通知和任务状态"
            aria-expanded={isNotificationOpen}
          >
            <Icon name="notifications" size={18} />
            {(props.syncSummary || props.taskProgress || props.statusMessage || props.errorMessage) && (
              <span className="absolute right-1.5 top-1.5 size-2 rounded-full border border-card bg-error" />
            )}
          </button>
          {isNotificationOpen && (
            <TopbarNotificationPanel
              syncSummary={props.syncSummary}
              taskProgress={props.taskProgress}
              statusMessage={props.statusMessage}
              errorMessage={props.errorMessage}
              onOpenDashboard={() => {
                setIsNotificationOpen(false);
                props.onNavigate('dashboard');
              }}
            />
          )}
          <button
            type="button"
            className="app-avatar"
            onClick={() => props.onNavigate('profile')}
            title={props.user?.login ?? '个人主页'}
            aria-label={props.user ? `打开 ${props.user.login} 的个人主页` : '打开个人主页'}
          >
            {props.user?.avatarUrl ? (
              <img src={props.user.avatarUrl} alt="" />
            ) : (
              props.user?.login?.[0]?.toUpperCase() ?? 'F'
            )}
          </button>
        </div>
      </header>

      <div className="app-body">
        <aside className="app-rail">
          <nav className="app-rail-nav" aria-label="主导航">
            {NAV_ITEMS.map((item) => (
              <RailButton
                key={item.key}
                item={item}
                active={activeNav === item.key}
                onClick={() => props.onNavigate(item.key)}
              />
            ))}
            <div className="app-rail-spacer" />
            <RailButton
              item={SETTINGS_ITEM}
              active={activeNav === SETTINGS_ITEM.key}
              onClick={() => props.onNavigate(SETTINGS_ITEM.key)}
            />
          </nav>
        </aside>
        <div className="app-content">
          {(props.errorMessage || props.statusMessage) && (
            <GlobalStatusBanner
              type={props.errorMessage ? 'error' : 'success'}
              message={props.errorMessage ?? props.statusMessage ?? ''}
            />
          )}
          {props.taskProgress && (
            <div className="shrink-0 border-b border-border bg-card px-3 py-2 sm:px-4">
              <TaskProgressCard
                progress={props.taskProgress}
                onRetry={props.onRetryTask}
                retryLabel={props.retryTaskLabel}
                isRetrying={props.isRetryingTask}
              />
            </div>
          )}
          <main className="app-main">{props.children}</main>
        </div>
      </div>
    </div>
  );
}

function RailButton(props: { item: NavItem; active: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      className="app-rail-item"
      data-active={props.active ? 'true' : undefined}
      aria-current={props.active ? 'page' : undefined}
      title={props.item.label}
      onClick={props.onClick}
    >
      <Icon name={props.item.icon} size={20} fill={props.active} />
      <span>{props.item.label}</span>
    </button>
  );
}

function GlobalStatusBanner(props: { type: 'success' | 'error'; message: string }) {
  const isError = props.type === 'error';

  return (
    <div className="shrink-0 border-b border-border bg-card px-3 py-2 sm:px-4">
      <div
        className={`flex min-w-0 items-start gap-2 rounded-lg border px-3 py-2 text-sm ${
          isError
            ? 'border-error/25 bg-error/10 text-error'
            : 'border-success/25 bg-success/10 text-success'
        }`}
        role={isError ? 'alert' : 'status'}
      >
        <Icon name={isError ? 'error' : 'check_circle'} size={18} className="mt-0.5 shrink-0" />
        <p className="min-w-0 flex-1 break-words font-body-md leading-relaxed">{props.message}</p>
      </div>
    </div>
  );
}

function TopbarQuickActionsMenu(props: {
  hasGitHubUser: boolean;
  isSyncing: boolean;
  onSyncStars: () => void;
  onOpenSettings: () => void;
  onFetchReadmes: () => void;
  isFetchingReadmes: boolean;
  onBatchGenerateAiDocuments: () => void;
  isBatchGeneratingAiDocuments: boolean;
  onGenerateAiTagNetwork: () => void;
  isGeneratingTagNetwork: boolean;
  onCheckForUpdate: () => void;
  isCheckingUpdate: boolean;
  searchQuery: string;
  onSearchQueryChange: (value: string) => void;
  onGlobalSearchSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  const needsGitHub = !props.hasGitHubUser;

  return (
    <div className="fixed right-3 top-[46px] z-50 w-[calc(100vw-1.5rem)] max-w-[360px] rounded-lg border border-border bg-card p-2 shadow-lg sm:right-4">
      <form className="border-b border-border px-2 pb-2" onSubmit={props.onGlobalSearchSubmit}>
        <p className="px-0.5 pb-1 text-xs font-medium text-on-surface">全局搜索</p>
        <input
          value={props.searchQuery}
          onChange={(event) => props.onSearchQueryChange(event.target.value)}
          placeholder="在全部仓库中搜索"
          aria-label="全局搜索"
          className="h-8 w-full rounded-md border border-input bg-card px-2 text-sm text-on-surface outline-none focus:border-primary"
        />
      </form>
      <div className="px-2 pb-2 pt-2">
        <p className="font-body-md text-sm font-semibold text-on-surface">快速操作</p>
        <p className="mt-0.5 text-xs text-on-surface-variant">常用批量任务会在后台执行。</p>
      </div>
      <div className="space-y-1">
        <QuickActionMenuItem
          icon="sync"
          label="同步 Stars"
          description={props.hasGitHubUser ? '拉取 GitHub 最新收藏。' : '先连接 GitHub 账号。'}
          disabled={needsGitHub || props.isSyncing}
          loading={props.isSyncing}
          onClick={props.onSyncStars}
        />
        <QuickActionMenuItem
          icon="description"
          label="抓取 README"
          description={props.hasGitHubUser ? '补齐缺失或变化的 README。' : '先连接 GitHub 账号。'}
          disabled={needsGitHub || props.isFetchingReadmes}
          loading={props.isFetchingReadmes}
          onClick={props.onFetchReadmes}
        />
        <QuickActionMenuItem
          icon="auto_awesome"
          label="增量 AI 解析"
          description="为已有 README 生成摘要、关键词和推荐标签。（会消耗 AI 额度）"
          disabled={needsGitHub || props.isBatchGeneratingAiDocuments}
          loading={props.isBatchGeneratingAiDocuments}
          onClick={props.onBatchGenerateAiDocuments}
        />
        <QuickActionMenuItem
          icon="hub"
          label="生成标签网络"
          description="用 AI 分析仓库之间的标签关系。（会消耗 AI 额度）"
          disabled={needsGitHub || props.isGeneratingTagNetwork}
          loading={props.isGeneratingTagNetwork}
          onClick={props.onGenerateAiTagNetwork}
        />
        <QuickActionMenuItem
          icon="system_update_alt"
          label="检查更新"
          description="查看是否有新的桌面应用版本。"
          disabled={props.isCheckingUpdate}
          loading={props.isCheckingUpdate}
          onClick={props.onCheckForUpdate}
        />
      </div>
      {needsGitHub && (
        <button
          type="button"
          onClick={props.onOpenSettings}
          className="mt-2 flex w-full items-center justify-center gap-1.5 rounded-lg border border-primary/25 bg-primary/10 px-3 py-2 text-xs font-medium text-primary transition-colors hover:bg-primary/15"
        >
          <Icon name="settings" size={15} />
          去设置连接 GitHub
        </button>
      )}
    </div>
  );
}

function QuickActionMenuItem(props: {
  icon: string;
  label: string;
  description: string;
  disabled: boolean;
  loading: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={props.onClick}
      disabled={props.disabled}
      className="flex w-full items-start gap-3 rounded-lg px-2.5 py-2 text-left transition-colors hover:bg-surface-container-low disabled:cursor-not-allowed disabled:opacity-55"
    >
      <span className="mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
        <Icon name={props.loading ? 'progress_activity' : props.icon} size={17} className={props.loading ? 'animate-spin' : ''} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block text-sm font-medium text-on-surface">{props.loading ? `${props.label}中` : props.label}</span>
        <span className="mt-0.5 block text-xs leading-relaxed text-on-surface-variant">{props.description}</span>
      </span>
    </button>
  );
}

function TopbarNotificationPanel(props: {
  syncSummary: StarSyncSummary | null;
  taskProgress: TaskProgressEvent | null;
  statusMessage: string | null;
  errorMessage: string | null;
  onOpenDashboard: () => void;
}) {
  const hasContent = props.syncSummary || props.taskProgress || props.statusMessage || props.errorMessage;

  return (
    <div className="fixed right-3 top-[46px] z-50 w-[calc(100vw-1.5rem)] max-w-sm rounded-lg border border-border bg-card p-3 shadow-lg sm:right-4">
      <div className="mb-2 flex items-center justify-between gap-3">
        <p className="font-body-md text-sm font-semibold text-on-surface">通知和任务</p>
        <button
          type="button"
          onClick={props.onOpenDashboard}
          className="flex h-8 items-center justify-center gap-1.5 rounded-lg px-2 text-xs font-medium text-primary transition-colors hover:bg-primary/10"
          title="打开仪表盘"
          aria-label="打开仪表盘"
        >
          <Icon name="dashboard" size={17} />
          仪表盘
        </button>
      </div>
      {!hasContent && (
        <div className="rounded-lg border border-outline-variant/25 bg-surface-container-low px-3 py-4 text-center text-sm text-on-surface-variant">
          暂无新的通知。
        </div>
      )}
      <div className="space-y-2">
        {props.errorMessage && (
          <TopbarNoticeItem
            icon="error"
            tone="error"
            title="需要处理"
            message={props.errorMessage}
          />
        )}
        {!props.errorMessage && props.statusMessage && (
          <TopbarNoticeItem
            icon="check_circle"
            tone="success"
            title="操作完成"
            message={props.statusMessage}
          />
        )}
        {props.taskProgress && (
          <TopbarNoticeItem
            icon={props.taskProgress.status === 'running' ? 'progress_activity' : 'task_alt'}
            tone={props.taskProgress.status === 'failed' ? 'error' : props.taskProgress.status === 'partial' ? 'warning' : 'primary'}
            title={props.taskProgress.status === 'running' ? '任务执行中' : '最近任务'}
            message={props.taskProgress.message}
            spinning={props.taskProgress.status === 'running'}
          />
        )}
        {props.syncSummary && (
          <TopbarNoticeItem
            icon="sync_saved_locally"
            tone="primary"
            title="最近同步"
            message={`活跃 ${props.syncSummary.activeCount} 个，新增 ${props.syncSummary.createdCount} 个，更新 ${props.syncSummary.updatedCount} 个，移除 ${props.syncSummary.removedCount} 个。`}
          />
        )}
      </div>
    </div>
  );
}

function TopbarNoticeItem(props: {
  icon: string;
  tone: 'primary' | 'success' | 'warning' | 'error';
  title: string;
  message: string;
  spinning?: boolean;
}) {
  const toneClass = {
    primary: 'bg-primary/10 text-primary',
    success: 'bg-success/10 text-success',
    warning: 'bg-warning/10 text-warning',
    error: 'bg-error/10 text-error',
  }[props.tone];

  return (
    <div className="flex min-w-0 gap-2 rounded-lg border border-outline-variant/25 bg-surface-container-low px-3 py-2">
      <span className={`mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-lg ${toneClass}`}>
        <Icon name={props.icon} size={16} className={props.spinning ? 'animate-spin' : ''} />
      </span>
      <div className="min-w-0 flex-1">
        <p className="text-xs font-semibold text-on-surface">{props.title}</p>
        <p className="mt-0.5 line-clamp-3 text-xs leading-relaxed text-on-surface-variant">{props.message}</p>
      </div>
    </div>
  );
}

function TaskProgressCard(props: {
  progress: TaskProgressEvent;
  onRetry: (() => void) | null;
  retryLabel: string | null;
  isRetrying: boolean;
}) {
  const progress = props.progress;
  const hasKnownProgress = progress.total > 0;
  const safeCurrent = hasKnownProgress ? Math.min(progress.current, progress.total) : progress.current;
  const percentage = hasKnownProgress ? Math.min(100, Math.round((safeCurrent / progress.total) * 100)) : 0;
  const progressLabel = progress.taskId === 'generate-ai-tag-network'
    ? `已完成 ${safeCurrent}/${progress.total} 批 · ${percentage}%`
    : `${safeCurrent}/${progress.total} · ${percentage}%`;
  const isRunning = progress.status === 'running';
  const isFailed = progress.status === 'failed';
  const isPartial = progress.status === 'partial';
  const canRetry = (isFailed || isPartial) && Boolean(props.onRetry);
  const stageLabel = getTaskStageLabel(progress.stage);
  const iconName = progress.taskType === 'ai'
    ? 'auto_awesome'
    : progress.taskType === 'readme'
      ? 'description'
      : progress.taskType === 'backup'
        ? 'backup'
        : 'sync';

  return (
    <div className="rounded-lg border border-border bg-card p-3">
      <div className="flex items-start gap-2">
        <span
          className={`mt-0.5 flex size-7 items-center justify-center rounded-lg ${
            isFailed
              ? 'bg-error/10 text-error'
              : isPartial
                ? 'bg-warning/10 text-warning'
                : isRunning
                  ? 'bg-primary/10 text-primary'
                  : 'bg-success/10 text-success'
          }`}
        >
          <Icon name={isRunning ? 'progress_activity' : isPartial ? 'warning' : iconName} size={16} className={isRunning ? 'animate-spin' : ''} />
        </span>
        <div className="min-w-0 flex-1">
          <p className="truncate text-xs font-semibold text-on-surface">
            {isRunning ? '任务执行中' : isFailed ? '任务失败' : isPartial ? '任务部分完成' : '任务完成'}
          </p>
          <p className={`mt-0.5 text-[11px] leading-snug text-on-surface-variant ${isFailed || isPartial ? 'whitespace-pre-wrap break-words' : 'line-clamp-2'}`}>
            {progress.message}
          </p>
          {(stageLabel || progress.repositoryName) && (
            <div className="mt-2 flex min-w-0 flex-wrap gap-1.5 text-[10px] leading-none text-on-surface-variant/90">
              {stageLabel && (
                <span className="max-w-full rounded-full border border-outline-variant/35 bg-surface-container-low px-2 py-1">
                  阶段：{stageLabel}
                </span>
              )}
              {progress.repositoryName && (
                <span className="max-w-full truncate rounded-full border border-outline-variant/35 bg-surface-container-low px-2 py-1" title={progress.repositoryName}>
                  当前：{progress.repositoryName}
                </span>
              )}
            </div>
          )}
        </div>
      </div>
      {isRunning && (
        <div className="mt-3">
          <div className="h-1.5 overflow-hidden rounded-full bg-surface-container-high">
            {hasKnownProgress ? (
              <div className="h-full rounded-full bg-primary transition-all" style={{ width: `${percentage}%` }} />
            ) : (
              <div className="task-progress-indeterminate h-full w-1/3 rounded-full bg-primary" />
            )}
          </div>
          <p className="mt-1 text-right text-[10px] text-on-surface-variant">
            {hasKnownProgress ? progressLabel : '正在处理...'}
          </p>
        </div>
      )}
      {canRetry && (
        <button
          type="button"
          onClick={props.onRetry ?? undefined}
          disabled={props.isRetrying}
          className="mt-3 inline-flex w-full items-center justify-center gap-1.5 rounded-lg border border-primary/25 bg-primary/10 px-2.5 py-1.5 text-xs font-medium text-primary transition-colors hover:bg-primary/15 disabled:cursor-not-allowed disabled:opacity-60"
        >
          <Icon name={props.isRetrying ? 'progress_activity' : 'refresh'} size={14} className={props.isRetrying ? 'animate-spin' : ''} />
          {props.isRetrying ? '重试中' : props.retryLabel ?? '重试任务'}
        </button>
      )}
    </div>
  );
}

function getTaskStageLabel(stage: string) {
  switch (stage) {
    case 'auth':
      return '验证账号';
    case 'prepare':
      return '准备数据';
    case 'batch':
      return '批量处理';
    case 'check':
      return '检查仓库';
    case 'plan':
      return '生成计划';
    case 'fetch':
    case 'fetch-readme':
    case 'github-search':
      return '请求数据';
    case 'github-star':
      return '更新 Stars';
    case 'parse':
      return '解析数据';
    case 'save':
      return '写入本地';
    case 'summarize':
    case 'analyze':
      return 'AI 分析';
    case 'partial-failure':
      return '部分失败';
    case 'done':
      return '已完成';
    case 'error':
      return '失败';
    case 'request':
      return '准备中';
    default:
      return stage ? stage : '';
  }
}
