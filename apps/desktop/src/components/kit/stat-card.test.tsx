/// <reference types="node" />
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { StatCard } from '@/components/kit/stat-card';
import cardSource from '@/components/kit/stat-card.tsx?raw';

// kit.css 用 Vite 的 ?raw / ?inline 都会得到空字符串（CSS 管线把默认导出换成空的），
// 所以这里跟 automation-view.test.ts 的文案守卫一样，直接读源文件。
const kitCss = readFileSync(fileURLToPath(new URL('../../styles/kit.css', import.meta.url)), 'utf8');

describe('统计卡深色选中态', () => {
  it('选中态类名同时保留浅色底，并有 dark:bg-[ 开头的深色底', () => {
    expect(cardSource).toContain('bg-[hsl(var(--h)_100%_98.5%)]');
    expect(cardSource).toContain('dark:bg-[');
  });

  it('kit.css 有深色 hue-tile，浅色规则原样还在', () => {
    expect(kitCss).toContain('.dark .hue-tile');
    expect(kitCss).toContain('.hue-tile {\n  background: hsl(var(--h) 85% 94%);');
  });

  it('渲染选中的统计卡时 class 里有深色底，浅色底也还在', () => {
    const html = renderToStaticMarkup(<StatCard label="x" value={1} icon="inbox" active />);
    expect(html).toContain('dark:bg-[');
    expect(html).toContain('bg-[hsl(var(--h)_100%_98.5%)]');
  });
});
