import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { MapPage } from '@/pages/v2/map';
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
