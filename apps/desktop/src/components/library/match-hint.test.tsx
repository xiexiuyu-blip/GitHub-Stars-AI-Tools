import type { ReactNode } from 'react';
import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { MATCH_STAR_HINTS } from '@/lib/match-hint';
import { MatchHint } from './match-hint';

function host(node: ReactNode): string {
  return renderToStaticMarkup(<div>{node}</div>);
}

function barClasses(html: string): string[] {
  const span = html.match(/<span role="img"[^>]*>[\s\S]*?<\/span>/)?.[0] ?? '';
  return [...span.matchAll(/<i class="([^"]*)"/g)].map((match) => match[1]);
}

describe('命中原因和匹配度', () => {
  it('三档加两个原因：文字、读屏标签，以及三根亮条', () => {
    const html = renderToStaticMarkup(<MatchHint reasons={['项目名', '中文说明']} stars={3} />);
    expect(html).toContain('命中：项目名 / 中文说明');
    expect(html).toContain('role="img"');
    expect(html).toContain('aria-label="匹配度 3 / 4"');
    const classes = barClasses(html);
    expect(classes).toHaveLength(4);
    expect(classes.filter((name) => name.split(/\s+/).includes('bg-primary'))).toHaveLength(3);
    expect(classes.filter((name) => name.split(/\s+/).includes('bg-border'))).toHaveLength(1);
  });

  it('悬停说明就是这一档的固定句子', () => {
    const html = renderToStaticMarkup(<MatchHint reasons={['项目名', '中文说明']} stars={3} />);
    expect(html).toContain(`title="${MATCH_STAR_HINTS[3]}"`);
  });

  it('没有原因也没有档位时，容器是空的', () => {
    expect(host(<MatchHint reasons={[]} stars={null} />)).toBe('<div></div>');
    expect(host(<MatchHint reasons={[]} stars={undefined} />)).toBe('<div></div>');
  });

  it('只有原因时不画竖条、不设悬停；只有档位时不写文字', () => {
    const reasonsOnly = renderToStaticMarkup(<MatchHint reasons={['项目名']} stars={null} />);
    expect(reasonsOnly).toContain('命中：项目名');
    expect(reasonsOnly).not.toContain('role="img"');
    expect(reasonsOnly).not.toContain('title=');

    const starsOnly = renderToStaticMarkup(<MatchHint reasons={[]} stars={2} />);
    expect(starsOnly).toContain('role="img"');
    expect(starsOnly).toContain('aria-label="匹配度 2 / 4"');
    expect(starsOnly).not.toContain('命中：');
    expect(starsOnly).toContain(`title="${MATCH_STAR_HINTS[2]}"`);
  });

  it('亮条从左往右，从最矮的那根开始', () => {
    for (const stars of [1, 2, 3, 4] as const) {
      const classes = barClasses(renderToStaticMarkup(<MatchHint reasons={[]} stars={stars} />));
      expect(classes, `档位 ${stars}`).toHaveLength(4);
      classes.forEach((name, index) => {
        const tokens = name.split(/\s+/);
        const lit = index < stars;
        expect(tokens.includes('bg-primary'), `档位 ${stars} 第 ${index + 1} 根 ${name}`).toBe(lit);
        expect(tokens.includes('bg-border'), `档位 ${stars} 第 ${index + 1} 根 ${name}`).toBe(!lit);
      });
    }
  });

  it('四根竖条从左到右依次是 6、8、10、12 像素', () => {
    const classes = barClasses(renderToStaticMarkup(<MatchHint reasons={[]} stars={4} />));
    const heights = ['h-[6px]', 'h-[8px]', 'h-[10px]', 'h-[12px]'];
    expect(classes).toHaveLength(heights.length);
    classes.forEach((name, index) => {
      expect(name.split(/\s+/).includes(heights[index]), `第 ${index + 1} 根 ${name}`).toBe(true);
    });
  });

  it('档位无效时按没有档位处理', () => {
    for (const stars of [5, 0, '3' as unknown as number]) {
      const withReason = renderToStaticMarkup(<MatchHint reasons={['项目名']} stars={stars} />);
      expect(withReason, String(stars)).toContain('命中：项目名');
      expect(withReason, String(stars)).not.toContain('role="img"');
      expect(withReason, String(stars)).not.toContain('title=');
      expect(host(<MatchHint reasons={[]} stars={stars} />), String(stars)).toBe('<div></div>');
    }
  });
});
