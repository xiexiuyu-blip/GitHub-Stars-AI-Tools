import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { ToastProvider } from '@/components/kit';
import { TodayPage } from '@/pages/v2/today';
// 页面源码原文：守住「叫法只在一处定义」（Vite 的 ?raw，不用 node 的类型）
import todaySource from '../../pages/v2/today.tsx?raw';

/** 今日页整页渲染一次（服务端渲染不跑 effect，所以是刚打开、还在读取的样子）：统计卡的名字都在。 */
function renderPage(): string {
  return renderToStaticMarkup(
    <ToastProvider>
      <TodayPage onOpenLibrary={() => {}} onOpenCapture={() => {}} onOpenSolve={() => {}} />
    </ToastProvider>,
  );
}

function textOf(html: string): string {
  return html.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim();
}

describe('今日页：最近 7 天新收藏', () => {
  it('统计卡叫「最近 7 天新收藏」，页面上没有「本周」「这周」（数据是最近 7 天，不是自然周）', () => {
    const text = textOf(renderPage());
    expect(text).toContain('最近 7 天新收藏');
    expect(text).not.toContain('本周');
    expect(text).not.toContain('这周');
    expect(text).not.toContain('这一周');
  });

  it('另外三张统计卡不变：待 AI 整理（还没有中文说明的收藏）、想试、在用（你标的）', () => {
    const text = textOf(renderPage());
    expect(text).toContain('待 AI 整理');
    expect(text).toContain('还没有中文说明的收藏');
    expect(text).toContain('想试');
    expect(text).toContain('在用');
    expect(text).toContain('你标的');
  });

  it('页面源码里叫法只在 item-format 里定义一次：统计卡、列表标题都引用它，不再自己写「本周新收藏」', () => {
    expect(todaySource).not.toContain('本周新收藏');
    expect(todaySource).not.toContain('这一周');
    expect(todaySource.match(/WEEK_NEW_LABEL/g)?.length).toBeGreaterThanOrEqual(3);
    expect(todaySource).toContain('todayEmptyCopy(filter)');
  });
});
