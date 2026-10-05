import { isValidElement, type ReactElement, type ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import type { PackFormat } from '@/lib/api';
import { formatOptions } from '@/lib/solve-view';
import { FormatPicker } from './format-picker';

type AnyElement = ReactElement<Record<string, unknown>>;

/** 在没有挂到页面上的元素树里找出所有满足条件的元素（FormatPicker 是纯函数组件，可以直接调用）。 */
function findAll(node: ReactNode, match: (element: AnyElement) => boolean, found: AnyElement[] = []): AnyElement[] {
  if (Array.isArray(node)) {
    for (const child of node) findAll(child as ReactNode, match, found);
    return found;
  }
  if (!isValidElement(node)) return found;
  const element = node as AnyElement;
  if (match(element)) found.push(element);
  findAll(element.props.children as ReactNode, match, found);
  return found;
}

function render(value: PackFormat): string {
  return renderToStaticMarkup(<FormatPicker options={formatOptions} value={value} onChange={() => undefined} />);
}

function inputsOf(markup: string): string[] {
  return markup.match(/<input[^>]*>/g) ?? [];
}

function attr(tag: string, name: string): string | null {
  return new RegExp(`\\s${name}="([^"]*)"`).exec(tag)?.[1] ?? null;
}

function isChecked(tag: string): boolean {
  return /\schecked(=""|\s|\/|>)/.test(tag);
}

describe('FormatPicker：格式选项竖排成单选列表', () => {
  it('渲染出三个选项，每个是真正的单选框，value 还是 markdown / agents / json', () => {
    const markup = render('markdown');
    const inputs = inputsOf(markup);
    expect(inputs).toHaveLength(3);
    expect(inputs.map((tag) => attr(tag, 'type'))).toEqual(['radio', 'radio', 'radio']);
    expect(inputs.map((tag) => attr(tag, 'value'))).toEqual(['markdown', 'agents', 'json']);
    // 读屏读的是完整的一句「名字（说明）」，不是把名字和说明粘在一起
    expect(inputs.map((tag) => attr(tag, 'aria-label'))).toEqual(formatOptions.map((option) => option.label));
    expect((markup.match(/<label[ >]/g) ?? [])).toHaveLength(3);
    expect(markup).toContain('role="radiogroup"');
    expect(markup).toContain('aria-label="格式"');
  });

  it('三个单选框共用一个 name（浏览器才会把它们当一组：Tab 进一次，方向键换）', () => {
    const names = inputsOf(render('markdown')).map((tag) => attr(tag, 'name'));
    expect(names[0]).toBeTruthy();
    expect(new Set(names).size).toBe(1);
  });

  it('每一项一行：名字 + 一句说明都显示出来', () => {
    const markup = render('markdown');
    for (const option of formatOptions) {
      expect(markup).toContain(option.name);
      expect(markup).toContain(option.note);
    }
    expect(markup).toContain('Markdown，直接粘给 Codex，默认');
    expect(markup).toContain('AGENTS.md，存成文件放进工作文件夹');
    expect(markup).toContain('JSON，一般不用选');
  });

  it('没有不换行的按钮：不用 button，也没有 whitespace-nowrap，长说明会自己换行', () => {
    const markup = render('agents');
    expect(markup).not.toContain('<button');
    expect(markup).not.toContain('whitespace-nowrap');
    expect(markup).toContain('break-words');
  });

  it('只有当前选中的那一项是勾上的，并且有底色', () => {
    for (const value of ['markdown', 'agents', 'json'] as const) {
      const markup = render(value);
      const checked = inputsOf(markup).filter(isChecked).map((tag) => attr(tag, 'value'));
      expect(checked).toEqual([value]);
      expect((markup.match(/bg-accent/g) ?? [])).toHaveLength(1);
    }
  });

  it('键盘能用：单选框不是 disabled，也没有被移出 Tab 顺序', () => {
    for (const tag of inputsOf(render('json'))) {
      expect(tag).not.toContain('disabled');
      expect(tag).not.toContain('tabindex');
    }
  });

  it('选哪一项，onChange 就收到那一项的 value', () => {
    const received: PackFormat[] = [];
    const tree = FormatPicker({ options: formatOptions, value: 'markdown', onChange: (value) => received.push(value) }) as ReactNode;
    const radios = findAll(tree, (element) => element.type === 'input');
    expect(radios).toHaveLength(3);
    for (const radio of radios) (radio.props.onChange as () => void)();
    expect(received).toEqual(['markdown', 'agents', 'json']);
  });
});
