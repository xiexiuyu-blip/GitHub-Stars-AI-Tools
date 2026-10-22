import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { MaintenanceTag } from './maintenance-tag';

function textOf(html: string): string {
  return html.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim();
}

describe('维护状态标签', () => {
  it('各档文案和色调；不画活跃时活跃是空的；没有状态是空的；悬停是门槛说明', () => {
    const active = renderToStaticMarkup(<MaintenanceTag tier="active" showActive />);
    expect(textOf(active)).toBe('活跃');
    expect(active).toContain('bg-accent');
    expect(active).toContain('title="最近 3 个月内有更新"');
    expect(active).toContain('inline-flex shrink-0');

    const slow = renderToStaticMarkup(<MaintenanceTag tier="slow" showActive />);
    expect(textOf(slow)).toBe('放缓');
    expect(slow).toContain('bg-muted');
    expect(slow).toContain('title="3 到 12 个月没有更新"');

    const stale = renderToStaticMarkup(<MaintenanceTag tier="stale" />);
    expect(textOf(stale)).toBe('疑似停更');
    expect(stale).toContain('bg-error-container');
    expect(stale).toContain('title="超过 1 年没有更新，可能已经不维护了"');

    expect(renderToStaticMarkup(<MaintenanceTag tier="active" showActive={false} />)).toBe('');
    expect(renderToStaticMarkup(<MaintenanceTag tier="active" />)).toBe('');
    expect(renderToStaticMarkup(<MaintenanceTag tier={null} showActive />)).toBe('');
    expect(renderToStaticMarkup(<MaintenanceTag tier={null} />)).toBe('');
  });
});
