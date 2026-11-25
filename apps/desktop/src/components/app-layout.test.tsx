import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import type { GitHubUser, TaskProgressEvent } from '@/types';
// 源码原文：守住「欢迎引导和主界面的任务条是同一种写法」（Vite 的 ?raw，不用 node 的类型）
import welcomeSource from './welcome-flow.tsx?raw';
import appLayoutSource from './app-layout.tsx?raw';
import { AppLayout, TaskProgressCard } from './app-layout';

function textOf(html: string): string {
  return html
    .replace(/<span class="material-symbols-outlined[^>]*>[^<]*<\/span>/g, '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

const USER: GitHubUser = { id: 1, login: 'fox', name: 'Fox', avatarUrl: null, htmlUrl: 'https://github.com/fox' };

function renderLayout(user: GitHubUser | null, taskProgress: TaskProgressEvent | null = null): string {
  return renderToStaticMarkup(
    <AppLayout
      currentPage="today"
      onNavigate={() => {}}
      user={user}
      onSyncStars={() => {}}
      isSyncing={false}
      onFetchReadmes={() => {}}
      isFetchingReadmes={false}
      onOpenToolbox={() => {}}
      syncSummary={null}
      syncedAt={null}
      taskProgress={taskProgress}
      onRetryTask={null}
      retryTaskLabel={null}
      isRetryingTask={false}
      statusMessage={null}
      errorMessage={null}
      notificationOpenSignal={0}
      onOpenCommandPalette={() => {}}
    >
      <div />
    </AppLayout>,
  );
}

function task(patch: Partial<TaskProgressEvent> = {}): TaskProgressEvent {
  return {
    taskId: 'sync-stars',
    taskType: 'sync',
    status: 'running',
    stage: 'fetch',
    current: 3,
    total: 12,
    message: '正在读取第 3 页',
    repositoryName: null,
    ...patch,
  };
}

function renderCard(progress: TaskProgressEvent): string {
  return renderToStaticMarkup(<TaskProgressCard progress={progress} onRetry={null} retryLabel={null} isRetrying={false} />);
}

describe('顶栏「同步」按钮的悬停提示', () => {
  it('连着账号时写「从 GitHub 同步你最新收藏的工具」，和快速操作里的「同步你最新的收藏」对齐用词（不写「拉取」「项目」）', () => {
    const html = renderLayout(USER);
    expect(html).toContain('title="从 GitHub 同步你最新收藏的工具"');
    expect(html).not.toContain('拉取');
    expect(html).not.toContain('最新收藏的项目');
  });

  it('还没连接账号时说请先连接，不变', () => {
    expect(renderLayout(null)).toContain('title="请先连接 GitHub 账号"');
  });
});

describe('任务条：正在做什么', () => {
  it('跑着的时候写「正在做：读取收藏…（第 3 步，共 12 步）」，进度条右下角只写百分比', () => {
    const text = textOf(renderCard(task()));
    expect(text).toContain('正在做：读取收藏…（第 3 步，共 12 步）');
    expect(text).toContain('25%');
    expect(text).not.toContain('阶段');
    expect(text).not.toContain('请求数据');
    expect(text).not.toContain('3/12');
  });

  it('原来写「已完成 N/M 批」的那种任务也一样，不再有「批」', () => {
    const text = textOf(renderCard(task({ taskId: 'generate-ai-tag-network', stage: 'analyze', current: 2, total: 5 })));
    expect(text).toContain('正在做：写中文说明…（第 2 步，共 5 步）');
    expect(text).not.toContain('已完成');
    expect(text).not.toContain('批');
    expect(text).not.toContain('2/5');
  });

  it('总数不知道：不写步数，进度条下面写「正在处理…」', () => {
    const text = textOf(renderCard(task({ total: 0, current: 0 })));
    expect(text).toContain('正在做：读取收藏…');
    expect(text).not.toContain('步');
    expect(text).toContain('正在处理…');
  });

  it('当前正在处理的工具名仍在（「当前：」）', () => {
    expect(textOf(renderCard(task({ repositoryName: 'fox/tool' })))).toContain('当前：fox/tool');
  });

  it('已经结束的任务：标题说结果，不再多一条「正在做」或阶段；失败也不露英文代号', () => {
    const done = textOf(renderCard(task({ status: 'succeeded', stage: 'done', current: 12, total: 12 })));
    expect(done).toContain('任务完成');
    expect(done).not.toContain('正在做');
    expect(done).not.toContain('阶段');
    const failed = textOf(renderCard(task({ status: 'failed', stage: 'error', message: '同步没完成' })));
    expect(failed).toContain('任务失败');
    expect(failed).not.toContain('阶段');
    expect(failed).not.toContain('error');
  });

  it('顶栏整页渲染时，任务条也是这个写法', () => {
    const text = textOf(renderLayout(USER, task({ stage: 'save', current: 7, total: 8 })));
    expect(text).toContain('正在做：保存到这台电脑…（第 7 步，共 8 步）');
    expect(text).not.toContain('阶段：');
  });
});

describe('欢迎引导里的任务条和主界面是同一种写法', () => {
  it('用同一个函数，不再自己写「阶段：」「3/12 · 25%」', () => {
    expect(welcomeSource).toContain('taskDoingText(progress)');
    expect(welcomeSource).toContain('taskPercentText(progress)');
    expect(welcomeSource).not.toContain('阶段：');
    expect(welcomeSource).not.toContain('请求数据');
    expect(welcomeSource).not.toContain('写入本地');
    expect(welcomeSource).not.toContain('${safeCurrent}/${progress.total}');
  });
});

describe('顶栏快速操作菜单精简', () => {
  it('不再包含重复搜索框、重复同步、旧版批量生成和失效的检查更新', () => {
    expect(appLayoutSource).toContain('下载项目说明');
    expect(appLayoutSource).toContain('自动整理与备份');
    expect(appLayoutSource).not.toContain('检查更新');
    expect(appLayoutSource).not.toContain('给没有说明的收藏写中文说明');
    expect(appLayoutSource).not.toContain('同步我的收藏');
  });
});
