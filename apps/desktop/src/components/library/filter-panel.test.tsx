import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import type { ReadingGroup } from '@/lib/contracts';
import { formOptions, type ScenarioFilterRow } from '@/lib/library-filters';
import { FilterPanel } from './filter-panel';
import { ThinTag } from './thin-tag';

function textOf(html: string): string {
  return html
    .replace(/<span class="material-symbols-outlined[^>]*>[^<]*<\/span>/g, '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

const READING: Record<ReadingGroup, number> = { unseen: 5, want: 2, tried: 1, in_use: 3, dropped: 0 };

function render(scenarios: ScenarioFilterRow[]): string {
  return renderToStaticMarkup(
    <FilterPanel
      scenarios={scenarios}
      scenarioStatus="ready"
      readingCounts={READING}
      forms={formOptions({ skill: 3, cli: 2, app: 1 })}
      countsKnown
      scenario={null}
      reading={null}
      form={null}
      onScenario={() => {}}
      onReading={() => {}}
      onForm={() => {}}
    />,
  );
}

/** 找到某个场景那一行的 HTML（一个 <button> 到它的结束标签）。 */
function rowOf(html: string, nameZh: string): string {
  const match = new RegExp(`<button[^>]*>(?:(?!</button>).)*>${nameZh}</span>(?:(?!</button>).)*</button>`).exec(html);
  expect(match, `找不到「${nameZh}」那一行`).not.toBeNull();
  return match?.[0] ?? '';
}

describe('资料库左栏：场景的「偏少」标签', () => {
  const scenarios: ScenarioFilterRow[] = [
    { id: 'scn-xhs', nameZh: '小红书运营与内容生产', count: 3, thin: true },
    { id: 'scn-ec', nameZh: '电商', count: 20, thin: false },
  ];

  it('标签文字只写「偏少」，不把「少于 14 个」写进标签里（左栏只有 200px 宽）', () => {
    const row = rowOf(render(scenarios), '小红书运营与内容生产');
    expect(textOf(row)).toContain('偏少');
    expect(textOf(row)).not.toContain('少于 14 个');
    expect(textOf(row)).not.toContain('偏少（');
  });

  it('「少于 14 个」放在鼠标悬停提示（title）里', () => {
    const row = rowOf(render(scenarios), '小红书运营与内容生产');
    expect(row).toContain('title="少于 14 个"');
  });

  it('标签包一层 shrink-0，窄的时候被挤的是场景名，不是标签', () => {
    const row = rowOf(render(scenarios), '小红书运营与内容生产');
    const wrapper = /<span class="([^"]*)" title="少于 14 个">/.exec(row);
    expect(wrapper).not.toBeNull();
    expect((wrapper?.[1] ?? '').split(/\s+/)).toContain('shrink-0');
  });

  it('场景名那一段有 truncate（太长截断）并且能缩（min-w-0 flex-1），所以不会被标签和个数挤没', () => {
    const row = rowOf(render(scenarios), '小红书运营与内容生产');
    const name = /<span class="([^"]*)">小红书运营与内容生产<\/span>/.exec(row);
    expect(name).not.toBeNull();
    const classes = (name?.[1] ?? '').split(/\s+/);
    expect(classes).toContain('truncate');
    expect(classes).toContain('min-w-0');
    expect(classes).toContain('flex-1');
  });

  it('个数也不缩：shrink-0', () => {
    const row = rowOf(render(scenarios), '小红书运营与内容生产');
    expect(row).toMatch(/<span class="[^"]*shrink-0[^"]*">3<\/span>/);
  });

  it('不偏少的场景没有标签，也没有悬停提示', () => {
    const row = rowOf(render(scenarios), '电商');
    expect(textOf(row)).not.toContain('偏少');
    expect(row).not.toContain('title=');
  });

  it('整个面板里只有偏少的那一个场景带这个提示', () => {
    expect(render(scenarios).split('title="少于 14 个"').length - 1).toBe(1);
  });
});

describe('ThinTag', () => {
  it('偏少 + 悬停提示 + shrink-0；蓝色的那种用在发现页', () => {
    const gray = renderToStaticMarkup(<ThinTag />);
    expect(gray).toMatch(/^<span class="inline-flex shrink-0" title="少于 14 个">/);
    expect(textOf(gray)).toBe('偏少');
    expect(gray).toContain('bg-muted');
    const blue = renderToStaticMarkup(<ThinTag tone="blue" />);
    expect(blue).toContain('bg-accent');
    expect(textOf(blue)).toBe('偏少');
  });
});

describe('资料库左栏：分组名和类型', () => {
  it('分组名叫「类型」，不叫「形态」', () => {
    const text = textOf(render([]));
    expect(text).toContain('类型');
    expect(text).not.toContain('形态');
  });

  it('类型用带解释的长写法：技能包（装进 Codex 用）、命令行工具（要让 Codex 帮你装）', () => {
    const text = textOf(render([]));
    expect(text).toContain('技能包（装进 Codex 用）');
    expect(text).toContain('命令行工具（要让 Codex 帮你装）');
    expect(text).toContain('应用');
  });

  it('三个分组都在：场景、我的状态、类型', () => {
    const text = textOf(render([]));
    expect(text.indexOf('场景')).toBeLessThan(text.indexOf('我的状态'));
    expect(text.indexOf('我的状态')).toBeLessThan(text.indexOf('类型'));
  });
});
