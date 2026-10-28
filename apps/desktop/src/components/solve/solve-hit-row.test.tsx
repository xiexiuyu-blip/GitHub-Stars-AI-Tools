import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { SolveHit } from '@/lib/contracts';
import { SolveHitRow } from './solve-hit-row';

function hit(partial: Partial<SolveHit> = {}): SolveHit {
  return {
    id: 'r1',
    fullName: 'fox/cover-kit',
    htmlUrl: 'https://github.com/fox/cover-kit',
    starsCount: 12,
    oneLiner: '做小红书封面的小工具',
    form: 'skill',
    scenarioIds: [],
    matched: ['封面'],
    missing: [],
    why: '对得上的地方：「封面」→名字里有',
    score: 1,
    fullMatch: true,
    hasCard: true,
    ...partial,
  };
}

function render(props: { hit?: Partial<SolveHit>; pressed?: boolean; isMain?: boolean; withMakeMain?: boolean }): string {
  return renderToStaticMarkup(
    <SolveHitRow
      hit={hit(props.hit)}
      intents={['封面']}
      pressed={props.pressed ?? false}
      isMain={props.isMain}
      onToggle={() => undefined}
      onOpen={() => undefined}
      onMakeMain={props.withMakeMain ? () => undefined : undefined}
    />,
  );
}

describe('SolveHitRow：找方案里的一行', () => {
  afterEach(() => vi.useRealTimers());

  it('复用维护边界，未知和活跃不加警告；已归档优先，也保留旧草稿兼容', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-10-22T12:00:00Z'));
    const cases = [
      { pushedAt: '2026-10-23T12:00:00Z', expected: null },
      { pushedAt: '2026-07-24T12:00:00Z', expected: null },
      { pushedAt: '2026-07-24T11:59:59Z', expected: '放缓' },
      { pushedAt: '2025-10-22T12:00:00Z', expected: '放缓' },
      { pushedAt: '2025-10-22T11:59:59Z', expected: '疑似停更' },
      { pushedAt: null, expected: null },
      { pushedAt: undefined, expected: null },
      { pushedAt: 'not-a-date', expected: null },
    ];
    for (const { pushedAt, expected } of cases) {
      const markup = render({ hit: { pushedAt, archived: null }, pressed: true });
      for (const label of ['放缓', '疑似停更', '已归档']) {
        expect(markup.includes(label), `${pushedAt}: ${label}`).toBe(label === expected);
      }
      expect(markup).toContain('aria-pressed="true"');
      expect(markup).not.toContain('disabled');
    }
    for (const pushedAt of [undefined, 'not-a-date', '2026-10-22T12:00:00Z', '2021-01-01T00:00:00Z']) {
      const markup = render({ hit: { pushedAt, archived: true, hasCard: false }, pressed: false });
      expect(markup).toContain('已归档');
      expect(markup).toContain('待 AI 整理');
      expect(markup).not.toContain('疑似停更');
      expect(markup).not.toContain('放缓');
      expect(markup).toContain('aria-pressed="false"');
      expect(markup).not.toContain('disabled');
    }
  });

  it('还没有中文说明的，标签写「待 AI 整理」（和今日、资料库统一），不写「待整理」', () => {
    const pending = render({ hit: { hasCard: false } });
    expect(pending).toContain('待 AI 整理');
    expect(pending).not.toContain('待整理');
    expect(pending).not.toContain('还没有中文说明');
    expect(render({ hit: { hasCard: true } })).not.toContain('待 AI 整理');
  });

  it('理由句直接显示，开头就是「对得上的地方：」，不再叠一个「为什么相关」', () => {
    const markup = render({});
    expect(markup).toContain('对得上的地方：「封面」→名字里有');
    expect(markup).not.toContain('为什么相关');
  });

  it('主参考旁边说明它是什么：Codex 会优先用它，其余是备选', () => {
    const main = render({ pressed: true, isMain: true });
    expect(main).toContain('主参考');
    expect(main).toContain('Codex 会优先用它，其余是备选');
    // 没勾的、或勾了但不是主参考的，不显示这句
    expect(render({ pressed: false, isMain: true })).not.toContain('Codex 会优先用它');
    const alternative = render({ pressed: true, isMain: false, withMakeMain: true });
    expect(alternative).not.toContain('Codex 会优先用它');
    expect(alternative).toContain('设为主参考');
  });

  it('勾选按钮的提示和读屏文字都叫「交接单」', () => {
    const off = render({ pressed: false });
    expect(off).toContain('aria-label="加入交接单 fox/cover-kit"');
    expect(off).toContain('title="加入交接单"');
    const on = render({ pressed: true });
    expect(on).toContain('aria-label="从交接单移除 fox/cover-kit"');
    expect(on).toContain('title="从交接单移除"');
    for (const markup of [off, on]) {
      expect(markup).not.toContain('参考包');
    }
  });
});
