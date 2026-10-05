import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { MapBlocks, MapPage } from '@/pages/v2/map';
import SOURCE from '@/pages/v2/map.tsx?raw';

/** 资料库右边那个子页签（原来叫「地图」，现在叫「按类型看」）。它只是给人看的，不能改库。SOURCE 是页面源码原文，用来守住「不再引用合并命令」。 */

function textOf(html: string): string {
  return html.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim();
}

describe('按类型看（原地图页）', () => {
  const html = renderToStaticMarkup(<MapPage />);

  it('页内标题叫「按类型看」，说明写清它和「场景」的区别，以及目前只能看不能改', () => {
    const text = textOf(html);
    expect(text).toContain('按类型看');
    expect(text).toContain('按「它是什么」给你的收藏分的类');
    expect(text).toContain('场景是「你拿它做什么」');
    expect(text).toContain('目前只能看，不能改。');
    expect(text).not.toContain('地图');
  });

  it('没有会直接合并分类的按钮：页面里一个按钮都没有，也没有「合并」', () => {
    expect(html).not.toContain('<button');
    expect(textOf(html)).not.toContain('合并');
  });

  it('不再说「规则初分」「锁定」这些开发词', () => {
    const text = textOf(html);
    expect(text).not.toContain('规则初分');
    expect(text).not.toContain('锁定');
    expect(text).not.toContain('概念');
  });

  it('页面代码里不再引用合并命令（命令本身留在后端，别删）', () => {
    expect(SOURCE).not.toContain('merge_knowledge_concepts');
    expect(SOURCE).not.toContain('合并');
  });

  it('页面只调用一个命令：读概览的 get_graph_overview，别的命令（尤其会写库的）一个都没有', () => {
    const commands = [...SOURCE.matchAll(/invoke(?:<[^>]*>)?\(\s*'([a-z_]+)'/g)].map((match) => match[1]);
    expect(commands).toEqual(['get_graph_overview']);
  });

  it('读出错时用错误色，不是原来那种把成功也画成红色的写法（成功提示已随按钮一起去掉）', () => {
    expect(SOURCE).toContain('text-error');
    expect(SOURCE).not.toContain('text-red-700');
    expect(SOURCE).not.toContain('已把前两个分类合并成概念');
  });
});

describe('按类型看：读取中、没有分类、有分类三种状态', () => {
  const blocks = [
    { slug: 'agent', nameZh: '智能助手', count: 12 },
    { slug: 'design', nameZh: '设计素材', count: 3 },
  ];

  it('刚打开、还在读取：写「读取中…」，不先说「没有分类」', () => {
    // 服务端渲染不跑 effect，页面就停在刚打开的样子
    const text = textOf(renderToStaticMarkup(<MapPage />));
    expect(text).toContain('读取中…');
    expect(text).not.toContain('还没有分类');
  });

  it('读完了、一个分类都没有：写「还没有分类。同步并整理之后这里会出现。」，不再是一片空白', () => {
    const text = textOf(renderToStaticMarkup(<MapBlocks loading={false} error={null} blocks={[]} />));
    expect(text).toBe('还没有分类。同步并整理之后这里会出现。');
    expect(text).not.toContain('读取中');
  });

  it('读取中的时候，哪怕手里有上一次的分类也先写「读取中…」', () => {
    expect(textOf(renderToStaticMarkup(<MapBlocks loading error={null} blocks={blocks} />))).toBe('读取中…');
  });

  it('有分类：每个分类一张卡，个数和名字都在，没有「读取中」「还没有分类」', () => {
    const html = renderToStaticMarkup(<MapBlocks loading={false} error={null} blocks={blocks} />);
    const text = textOf(html);
    expect(text).toContain('12');
    expect(text).toContain('智能助手');
    expect(text).toContain('3');
    expect(text).toContain('设计素材');
    expect(text).not.toContain('读取中');
    expect(text).not.toContain('还没有分类');
    expect(html).not.toContain('<button');
  });

  it('读出错：只显示错误（错误色），不说成「还没有分类」', () => {
    const html = renderToStaticMarkup(<MapBlocks loading={false} error="读不出来" blocks={[]} />);
    expect(textOf(html)).toBe('读不出来');
    expect(html).toContain('text-error');
    expect(textOf(html)).not.toContain('还没有分类');
  });

  it('页面接线：读完（成功或出错）都结束「读取中」，页面卸载后不再写状态', () => {
    expect(SOURCE).toContain('setLoading(false)');
    expect(SOURCE.match(/setLoading\(false\)/g)?.length).toBe(2);
    expect(SOURCE).toContain('cancelled');
  });
});
