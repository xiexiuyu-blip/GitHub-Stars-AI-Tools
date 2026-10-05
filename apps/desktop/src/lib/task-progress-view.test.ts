import { describe, expect, it } from 'vitest';
import type { TaskProgressEvent } from '@/types';
import { taskDoingLabel, taskDoingText, taskPercentText, taskStepText } from './task-progress-view';

function progress(patch: Partial<TaskProgressEvent> = {}): TaskProgressEvent {
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

describe('taskDoingText：任务条上「正在做什么」', () => {
  it('读取收藏：「正在做：读取收藏…（第 3 步，共 12 步）」（审读第 47 条的写法）', () => {
    expect(taskDoingText(progress())).toBe('正在做：读取收藏…（第 3 步，共 12 步）');
  });

  it('不再有「阶段：」「请求数据」「写入本地」「已完成 3/12 批」这些开发话', () => {
    for (const stage of ['auth', 'queue', 'prepare', 'batch', 'check', 'plan', 'fetch', 'fetch-readme', 'github-search', 'github-star', 'parse', 'save', 'summarize', 'analyze', 'request']) {
      const text = taskDoingText(progress({ stage })) ?? '';
      expect(text, stage).toContain('正在做：');
      expect(text, stage).not.toContain('阶段');
      expect(text, stage).not.toContain('请求数据');
      expect(text, stage).not.toContain('写入本地');
      expect(text, stage).not.toMatch(/\d+\s*批/);
      expect(text, stage).toContain('（第 3 步，共 12 步）');
    }
  });

  it('各个阶段都有大白话：下载项目说明、写中文说明、保存到这台电脑……', () => {
    expect(taskDoingText(progress({ stage: 'fetch-readme' }))).toBe('正在做：下载项目说明…（第 3 步，共 12 步）');
    expect(taskDoingText(progress({ stage: 'summarize' }))).toBe('正在做：写中文说明…（第 3 步，共 12 步）');
    expect(taskDoingText(progress({ stage: 'analyze' }))).toBe('正在做：写中文说明…（第 3 步，共 12 步）');
    expect(taskDoingText(progress({ stage: 'save' }))).toBe('正在做：保存到这台电脑…（第 3 步，共 12 步）');
    expect(taskDoingText(progress({ stage: 'github-star' }))).toBe('正在做：更新收藏…（第 3 步，共 12 步）');
    expect(taskDoingLabel('queue')).toBe('排队等候');
  });

  it('总数不知道：只说在做什么，不写步数', () => {
    expect(taskDoingText(progress({ total: 0, current: 0 }))).toBe('正在做：读取收藏…');
    expect(taskDoingText(progress({ total: Number.NaN }))).toBe('正在做：读取收藏…');
  });

  it('阶段认不出（或没有）：不把英文代号露出来；有步数就只写步数，什么都没有就不显示', () => {
    expect(taskDoingText(progress({ stage: 'some-internal-stage' }))).toBe('第 3 步，共 12 步');
    expect(taskDoingText(progress({ stage: '' }))).toBe('第 3 步，共 12 步');
    expect(taskDoingText(progress({ stage: 'some-internal-stage', total: 0 }))).toBeNull();
    expect(taskDoingText(progress({ stage: 'some-internal-stage' }))).not.toContain('some-internal-stage');
  });

  it('已经结束的任务（完成 / 失败 / 部分完成）不再标「正在做」：标题已经说了结果', () => {
    for (const status of ['succeeded', 'failed', 'partial']) {
      expect(taskDoingText(progress({ status })), status).toBeNull();
    }
    expect(taskDoingLabel('done')).toBeNull();
    expect(taskDoingLabel('error')).toBeNull();
    expect(taskDoingLabel('partial-failure')).toBeNull();
  });

  it('步数不会写成「第 13 步，共 12 步」或负数；还没开始只说共多少步', () => {
    expect(taskStepText(13, 12)).toBe('第 12 步，共 12 步');
    expect(taskStepText(-2, 12)).toBe('共 12 步');
    expect(taskStepText(0, 12)).toBe('共 12 步');
    expect(taskStepText(Number.NaN, 12)).toBe('共 12 步');
    expect(taskStepText(3.9, 12.7)).toBe('第 3 步，共 12 步');
    expect(taskStepText(3, 0)).toBeNull();
    expect(taskStepText(3, -1)).toBeNull();
    expect(taskStepText(3, Number.POSITIVE_INFINITY)).toBeNull();
    expect(taskStepText(1, 0.4)).toBeNull();
  });
});

describe('taskPercentText：进度条右下角', () => {
  it('只写百分比，不再写「3/12」或「已完成 3/12 批」（步数已经在上面那一条里）', () => {
    expect(taskPercentText({ current: 3, total: 12 })).toBe('25%');
    expect(taskPercentText({ current: 12, total: 12 })).toBe('100%');
    expect(taskPercentText({ current: 15, total: 12 })).toBe('100%');
    expect(taskPercentText({ current: 0, total: 12 })).toBe('0%');
    expect(taskPercentText({ current: 3, total: 12 })).not.toContain('/');
  });

  it('总数不知道：没有百分比，页面写「正在处理…」', () => {
    expect(taskPercentText({ current: 3, total: 0 })).toBeNull();
    expect(taskPercentText({ current: 3, total: Number.NaN })).toBeNull();
  });
});
