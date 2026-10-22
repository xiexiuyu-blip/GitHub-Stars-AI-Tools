import { isValidElement, type ReactElement, type ReactNode } from 'react';
import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import type { ReadingGroup } from '@/lib/contracts';
import { formOptions, maintenanceOptions, type ScenarioFilterRow } from '@/lib/library-filters';
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
      maintenance={[]}
      countsKnown
      scenario={null}
      reading={null}
      form={null}
      maintenanceTier={null}
      onScenario={() => {}}
      onReading={() => {}}
      onForm={() => {}}
      onMaintenance={() => {}}
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

  it('标签文字只写「偏少」，不把「14 个以内」写进标签里（左栏只有 200px 宽）', () => {
    const row = rowOf(render(scenarios), '小红书运营与内容生产');
    expect(textOf(row)).toContain('偏少');
    expect(textOf(row)).not.toContain('14 个以内');
    expect(textOf(row)).not.toContain('偏少（');
  });

  it('「14 个以内」放在鼠标悬停提示（title）里', () => {
    const row = rowOf(render(scenarios), '小红书运营与内容生产');
    expect(row).toContain('title="14 个以内"');
  });

  it('标签包一层 shrink-0，窄的时候被挤的是场景名，不是标签', () => {
    const row = rowOf(render(scenarios), '小红书运营与内容生产');
    const wrapper = /<span class="([^"]*)" title="14 个以内">/.exec(row);
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
    expect(render(scenarios).split('title="14 个以内"').length - 1).toBe(1);
  });
});

describe('ThinTag', () => {
  it('偏少 + 悬停提示 + shrink-0；蓝色的那种用在发现页', () => {
    const gray = renderToStaticMarkup(<ThinTag />);
    expect(gray).toMatch(/^<span class="inline-flex shrink-0" title="14 个以内">/);
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

  it('类型行显示短名字 + 个数：技能 3、命令行 2、应用 1，不被截成半句', () => {
    const text = textOf(render([]));
    expect(text).toContain('技能');
    expect(text).toContain('命令行');
    expect(text).toContain('应用');
    // 长解释不在行里：200px 宽的左栏放不下，之前被截成「技能包（装进 Codex…」
    expect(text).not.toContain('技能包（');
    expect(text).not.toContain('命令行工具（');
    const skill = rowOf(render([]), '技能');
    expect(textOf(skill)).toBe('技能 3');
    expect(textOf(rowOf(render([]), '命令行'))).toBe('命令行 2');
    expect(textOf(rowOf(render([]), '应用'))).toBe('应用 1');
  });

  it('长解释放进悬停提示（title）：技能包（给 AI 助手装的扩展）、命令行工具（要让 Codex 帮你装）；应用没有解释，就没有提示', () => {
    const html = render([]);
    expect(rowOf(html, '技能')).toContain('title="技能包（给 AI 助手装的扩展）"');
    expect(rowOf(html, '命令行')).toContain('title="命令行工具（要让 Codex 帮你装）"');
    expect(rowOf(html, '应用')).not.toContain('title=');
    expect(html).not.toContain('装进 Codex 用');
  });

  it('三个分组都在：场景、我的状态、类型', () => {
    const text = textOf(render([]));
    expect(text.indexOf('场景')).toBeLessThan(text.indexOf('我的状态'));
    expect(text.indexOf('我的状态')).toBeLessThan(text.indexOf('类型'));
  });
});

function walk(node: ReactNode, visit: (el: ReactElement<unknown>) => void): void {
  if (node == null || typeof node === 'boolean' || typeof node === 'string' || typeof node === 'number') return;
  if (Array.isArray(node)) {
    for (const child of node) walk(child, visit);
    return;
  }
  if (!isValidElement(node) || !('type' in node)) return;
  const element = node as ReactElement<unknown>;
  if (typeof element.type === 'function') {
    walk((element.type as (props: unknown) => ReactNode)(element.props), visit);
    return;
  }
  visit(element);
  walk((element.props as { children?: ReactNode } | null)?.children, visit);
}

function hostText(node: ReactNode): string {
  if (node == null || typeof node === 'boolean') return '';
  if (typeof node === 'string' || typeof node === 'number') return String(node);
  if (Array.isArray(node)) return node.map(hostText).join('');
  if (!isValidElement(node)) return '';
  if (typeof node.type === 'function') return hostText((node.type as (props: unknown) => ReactNode)(node.props));
  return hostText((node.props as { children?: ReactNode }).children);
}

describe('资料库左栏：维护状态', () => {
  it('有分组、三项和个数，选中能点，个数为 0 的淡一点，个数未知显示省略号，悬停是门槛说明', () => {
    const calls: string[] = [];
    const rows = maintenanceOptions({ active: 0, slow: 0, stale: 2 });
    const element = (
      <FilterPanel
        scenarios={[]}
        scenarioStatus="ready"
        readingCounts={READING}
        forms={[]}
        maintenance={rows}
        countsKnown
        scenario={null}
        reading={null}
        form={null}
        maintenanceTier="slow"
        onScenario={() => {}}
        onReading={() => {}}
        onForm={() => {}}
        onMaintenance={(tier) => calls.push(tier)}
      />
    );
    const html = renderToStaticMarkup(element);
    const text = textOf(html);
    expect(text.indexOf('类型')).toBeLessThan(text.indexOf('维护状态'));
    expect(text.indexOf('活跃')).toBeLessThan(text.indexOf('放缓'));
    expect(text.indexOf('放缓')).toBeLessThan(text.indexOf('疑似停更'));
    expect(textOf(rowOf(html, '活跃'))).toBe('活跃 0');
    expect(textOf(rowOf(html, '放缓'))).toBe('放缓 0');
    expect(textOf(rowOf(html, '疑似停更'))).toBe('疑似停更 2');
    expect(rowOf(html, '放缓')).toContain('aria-pressed="true"');
    expect(rowOf(html, '活跃')).toContain('aria-pressed="false"');
    expect(rowOf(html, '活跃')).toContain('title="最近 3 个月内有更新"');
    expect(rowOf(html, '放缓')).toContain('title="3 到 12 个月没有更新"');
    expect(rowOf(html, '疑似停更')).toContain('title="超过 1 年没有更新，可能已经不维护了"');
    expect(rowOf(html, '活跃')).toContain('opacity-50');
    expect(rowOf(html, '放缓')).not.toContain('opacity-50');

    const buttons: ReactElement<unknown>[] = [];
    walk(element, (el) => {
      if (el.type === 'button') buttons.push(el);
    });
    const active = buttons.find((button) => hostText(button).includes('活跃'));
    const stale = buttons.find((button) => hostText(button).includes('疑似停更'));
    (active?.props as { onClick?: () => void } | null)?.onClick?.();
    (stale?.props as { onClick?: () => void } | null)?.onClick?.();
    expect(calls).toEqual(['active', 'stale']);

    const unknown = renderToStaticMarkup(
      <FilterPanel
        scenarios={[]}
        scenarioStatus="ready"
        readingCounts={READING}
        forms={[]}
        maintenance={rows}
        countsKnown={false}
        scenario={null}
        reading={null}
        form={null}
        maintenanceTier={null}
        onScenario={() => {}}
        onReading={() => {}}
        onForm={() => {}}
        onMaintenance={() => {}}
      />,
    );
    expect(textOf(rowOf(unknown, '疑似停更'))).toBe('疑似停更 …');
  });

  it('已归档有个数，选中能点，个数为 0 且未选中时淡一点，悬停带同步提醒', () => {
    const calls: string[] = [];
    const rows = maintenanceOptions({ active: 1, slow: 0, stale: 2, archived: 7 });
    const element = (
      <FilterPanel
        scenarios={[]}
        scenarioStatus="ready"
        readingCounts={READING}
        forms={[]}
        maintenance={rows}
        countsKnown
        scenario={null}
        reading={null}
        form={null}
        maintenanceTier="archived"
        onScenario={() => {}}
        onReading={() => {}}
        onForm={() => {}}
        onMaintenance={(tier) => calls.push(tier)}
      />
    );
    const html = renderToStaticMarkup(element);
    const text = textOf(html);
    expect(text.indexOf('疑似停更')).toBeLessThan(text.indexOf('已归档'));
    expect(textOf(rowOf(html, '已归档'))).toBe('已归档 7');
    expect(rowOf(html, '已归档')).toContain('aria-pressed="true"');
    expect(rowOf(html, '已归档')).toContain(
      'title="作者已在 GitHub 上把它归档：只读，不会再更新。旧数据点一次「同步」后才会标出来"',
    );

    const buttons: ReactElement<unknown>[] = [];
    walk(element, (el) => {
      if (el.type === 'button') buttons.push(el);
    });
    const archived = buttons.find((button) => hostText(button).includes('已归档'));
    (archived?.props as { onClick?: () => void } | null)?.onClick?.();
    expect(calls).toEqual(['archived']);

    const zero = renderToStaticMarkup(
      <FilterPanel
        scenarios={[]}
        scenarioStatus="ready"
        readingCounts={READING}
        forms={[]}
        maintenance={maintenanceOptions({ active: 1, slow: 1, stale: 1, archived: 0 })}
        countsKnown
        scenario={null}
        reading={null}
        form={null}
        maintenanceTier={null}
        onScenario={() => {}}
        onReading={() => {}}
        onForm={() => {}}
        onMaintenance={() => {}}
      />,
    );
    expect(textOf(rowOf(zero, '已归档'))).toBe('已归档 0');
    expect(rowOf(zero, '已归档')).toContain('aria-pressed="false"');
    expect(rowOf(zero, '已归档')).toContain('opacity-50');
  });
});
