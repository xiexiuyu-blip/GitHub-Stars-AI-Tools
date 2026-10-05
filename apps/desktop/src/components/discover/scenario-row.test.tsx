import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import type { DiscoverScenario } from '@/lib/contracts';
import { ScenarioRow } from './scenario-row';

function textOf(html: string): string {
  return html
    .replace(/<span class="material-symbols-outlined[^>]*>[^<]*<\/span>/g, '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

function make(overrides: Partial<DiscoverScenario> = {}): DiscoverScenario {
  return {
    id: 'scn-xhs',
    slug: 'xhs',
    nameZh: '小红书运营与内容生产',
    sortOrder: 1,
    myCount: 3,
    thin: true,
    candidateCount: 0,
    lastFetchedAt: null,
    ...overrides,
  };
}

function render(scenario: DiscoverScenario, active = false): string {
  return renderToStaticMarkup(<ScenarioRow scenario={scenario} active={active} onClick={() => {}} />);
}

describe('发现左栏：一行场景', () => {
  it('偏少的标签只写「偏少」，「14 个以内」在悬停提示里；标签 shrink-0', () => {
    const html = render(make());
    const wrapper = /<span class="([^"]*)" title="14 个以内">/.exec(html);
    expect(wrapper).not.toBeNull();
    expect((wrapper?.[1] ?? '').split(/\s+/)).toContain('shrink-0');
    expect(textOf(html)).toContain('偏少');
    expect(textOf(html)).not.toContain('偏少（');
    expect(textOf(html)).not.toContain('14 个以内');
  });

  it('场景名那一段 truncate、能缩，名字再长也不会把标签和个数挤出去', () => {
    const html = render(make({ nameZh: '一个特别特别特别特别特别特别长的场景名字' }));
    const name = /<span class="([^"]*)">一个特别特别特别特别特别特别长的场景名字<\/span>/.exec(html);
    expect(name).not.toBeNull();
    const classes = (name?.[1] ?? '').split(/\s+/);
    expect(classes).toEqual(expect.arrayContaining(['truncate', 'min-w-0', 'flex-1']));
  });

  it('没找过：写「还没找过」，悬停提示说还没去 GitHub 找过', () => {
    const html = render(make({ candidateCount: 0, lastFetchedAt: null }));
    expect(textOf(html)).toContain('还没找过');
    expect(html).toContain('title="还没去 GitHub 找过"');
    expect(textOf(html)).not.toContain('—');
  });

  it('找过但没有结果：和「还没找过」分开写，悬停提示是整句', () => {
    const html = render(make({ candidateCount: 0, lastFetchedAt: '2026-10-04T12:00:00Z' }));
    expect(textOf(html)).toContain('没有结果');
    expect(textOf(html)).not.toContain('还没找过');
    expect(html).toContain('title="找过了，这次没有结果"');
  });

  it('有候选：写个数', () => {
    const html = render(make({ candidateCount: 12, lastFetchedAt: '2026-10-04T12:00:00Z' }));
    expect(textOf(html)).toContain('12 个');
    expect(html).toContain('title="12 个你还没收藏的候选"');
  });

  it('个数那一格不缩，字也短（左栏只有 220px）', () => {
    const none = render(make({ candidateCount: 0, lastFetchedAt: '2026-10-04T12:00:00Z' }));
    const cell = /<span class="([^"]*)" title="找过了，这次没有结果">([^<]*)<\/span>/.exec(none);
    expect(cell).not.toBeNull();
    expect((cell?.[1] ?? '').split(/\s+/)).toContain('shrink-0');
    expect(Array.from(cell?.[2] ?? '').length).toBeLessThanOrEqual(4);
  });

  it('不偏少的场景没有标签；选中的那行 aria-pressed', () => {
    const html = render(make({ thin: false }), true);
    expect(textOf(html)).not.toContain('偏少');
    expect(html).not.toContain('title="14 个以内"');
    expect(html).toContain('aria-pressed="true"');
    expect(render(make({ thin: false }), false)).toContain('aria-pressed="false"');
  });
});
