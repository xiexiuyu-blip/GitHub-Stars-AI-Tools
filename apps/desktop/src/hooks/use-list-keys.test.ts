import { describe, expect, it } from 'vitest';
import { nextSelection, shouldIgnoreKey } from './use-list-keys';

const ids = ['a', 'b', 'c'];

describe('nextSelection', () => {
  it('列表为空返回 null', () => {
    expect(nextSelection([], null, 'j')).toBeNull();
    expect(nextSelection([], 'a', 'k')).toBeNull();
  });

  it('当前为空或已不在列表里时，j/k 都选第一个', () => {
    expect(nextSelection(ids, null, 'j')).toBe('a');
    expect(nextSelection(ids, null, 'k')).toBe('a');
    expect(nextSelection(ids, 'missing', 'j')).toBe('a');
  });

  it('j 向下，到最后一项停住，不绕回第一项', () => {
    expect(nextSelection(ids, 'a', 'j')).toBe('b');
    expect(nextSelection(ids, 'b', 'J')).toBe('c');
    expect(nextSelection(ids, 'c', 'j')).toBe('c');
  });

  it('k 向上，到第一项停住，不绕到最后', () => {
    expect(nextSelection(ids, 'c', 'k')).toBe('b');
    expect(nextSelection(ids, 'b', 'K')).toBe('a');
    expect(nextSelection(ids, 'a', 'k')).toBe('a');
  });

  it('ArrowDown 向下、ArrowUp 向上，到头停住，空选首个', () => {
    expect(nextSelection(ids, null, 'ArrowDown')).toBe('a');
    expect(nextSelection(ids, null, 'ArrowUp')).toBe('a');
    expect(nextSelection(ids, 'a', 'ArrowDown')).toBe('b');
    expect(nextSelection(ids, 'c', 'ArrowDown')).toBe('c');
    expect(nextSelection(ids, 'c', 'ArrowUp')).toBe('b');
    expect(nextSelection(ids, 'a', 'ArrowUp')).toBe('a');
  });

  it('其他键不移动', () => {
    expect(nextSelection(ids, 'b', 't')).toBeNull();
    expect(nextSelection(ids, 'b', 'ArrowLeft')).toBeNull();
    expect(nextSelection(ids, 'b', 'ArrowRight')).toBeNull();
    expect(nextSelection(ids, null, 'x')).toBeNull();
  });
});

describe('shouldIgnoreKey', () => {
  it('输入框、多行文本、下拉框忽略', () => {
    expect(shouldIgnoreKey({ tagName: 'INPUT' })).toBe(true);
    expect(shouldIgnoreKey({ tagName: 'textarea' })).toBe(true);
    expect(shouldIgnoreKey({ tagName: 'Select' })).toBe(true);
  });

  it('contenteditable 忽略', () => {
    expect(shouldIgnoreKey({ tagName: 'div', isContentEditable: true })).toBe(true);
    expect(shouldIgnoreKey({ tagName: 'div', isContentEditable: false })).toBe(false);
  });

  it('⌘、Ctrl、Alt 忽略，普通按键不忽略', () => {
    expect(shouldIgnoreKey({ tagName: 'div', metaKey: true })).toBe(true);
    expect(shouldIgnoreKey({ tagName: 'div', ctrlKey: true })).toBe(true);
    expect(shouldIgnoreKey({ tagName: 'div', altKey: true })).toBe(true);
    expect(shouldIgnoreKey({ tagName: 'button' })).toBe(false);
    expect(shouldIgnoreKey(null)).toBe(false);
    expect(shouldIgnoreKey(undefined)).toBe(false);
  });
});
