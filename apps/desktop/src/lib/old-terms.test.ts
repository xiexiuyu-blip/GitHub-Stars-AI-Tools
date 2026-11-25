import { describe, expect, it } from 'vitest';

/**
 * 文案守卫（卡 36 第 5 条）：新界面的源码里，用户看得见的文字不再出现这几个旧叫法。
 * 源码原文用 Vite 的 import.meta.glob + ?raw 读进来（不用 node 的类型），去掉注释之后再查，
 * 所以代码注释、变量名里讲数据模型时写「仓库」不算；写在字符串和界面文字里的才算。
 *
 * 不查的（都不是新界面里看得见的文字，也都在卡 36 的报告里列了出来）：
 * - 测试文件；
 * - 旧兼容工具文件（features/**，components/copy-link-button、app-shell，lib/repository）；
 * - lib/codex-prompts.ts：复制出去给 Codex 的话，不是界面上的字，按卡 31 / 32 / 33 的约定不改；
 * - pages/settings.tsx：设置页有自己的守卫（automation-view.test.ts），那里已经把藏起来的向量面板排除在外。
 */
const RAW = import.meta.glob(['../**/*.ts', '../**/*.tsx'], { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

const SKIPPED = [
  /\.test\.tsx?$/,
  /\/features\//,
  /\/components\/(copy-link-button|app-shell)\.tsx$/,
  /\/lib\/repository\.ts$/,
  /\/lib\/codex-prompts\.ts$/,
  /\/pages\/settings\.tsx$/,
];

/** 去掉注释，只留会显示给人看的字符串和 JSX 文字（注释里写旧叫法不算）。 */
function withoutComments(source: string): string {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/(^|[^:])\/\/[^\n]*/g, '$1');
}

const BANNED = ['工具箱', '地图', '参考包', '方向', '仓库', '理解卡', '增量整理', '全量整理'];

/** glob 的键是相对本文件的路径（./nav.ts、../App.tsx）：统一成 /src/… 开头的路径，方便按目录匹配。 */
const normalize = (key: string) => new URL(key, 'file:///src/lib/').pathname;

const files = Object.entries(RAW)
  .map(([path, source]) => [normalize(path), source] as const)
  .filter(([path]) => !SKIPPED.some((pattern) => pattern.test(path)))
  .map(([path, source]) => [path, withoutComments(source)] as const);

describe('文案守卫：新界面的源码里没有旧叫法', () => {
  it('确实读到了新界面的源码（不是空跑）', () => {
    const paths = files.map(([path]) => path);
    for (const expected of [
      '/src/lib/nav.ts',
      '/src/App.tsx',
      '/src/components/app-layout.tsx',
      '/src/components/library/detail-panels.tsx',
      '/src/pages/v2/library.tsx',
      '/src/pages/v2/today.tsx',
      '/src/pages/v2/solve.tsx',
      '/src/pages/v2/capture.tsx',
      '/src/pages/v2/toolbox.tsx',
    ]) {
      expect(paths, expected).toContain(expected);
    }
    // 旧页面、测试、复制给 Codex 的话不在检查范围里
    expect(paths).not.toContain('/src/pages/repositories.tsx');
    expect(paths).not.toContain('/src/lib/codex-prompts.ts');
    expect(paths.some((path) => path.endsWith('.test.ts') || path.endsWith('.test.tsx'))).toBe(false);
  });

  it.each(BANNED)('用户看得见的文字里没有「%s」', (term) => {
    const hits = files
      .flatMap(([path, text]) => text.split('\n')
        .map((line, index) => ({ path, line: index + 1, text: line.trim() }))
        .filter((row) => row.text.includes(term)));
    expect(hits, `旧叫法「${term}」还在：${hits.map((row) => `${row.path}:${row.line}`).join('、')}`).toEqual([]);
  });
});
