import { afterEach, describe, expect, it } from 'vitest';
import { withInitial } from './library-filters';
import { loadLibrarySession, saveLibrarySession, type LibrarySessionSnapshot } from './library-session';

function snapshot(overrides: Partial<LibrarySessionSnapshot> = {}): LibrarySessionSnapshot {
  const ui = withInitial('', null, null);
  return {
    ui: { ...ui, keyword: 'agent', reading: 'want' },
    keywordDraft: 'agent',
    selectedId: 'repo-42',
    scrollTop: 360,
    cardStateTouched: true,
    ...overrides,
  };
}

describe('library-session', () => {
  afterEach(() => {
    saveLibrarySession(null);
  });

  it('无初始覆盖条件时可保存并恢复筛选、选中项和滚动位置', () => {
    expect(loadLibrarySession(false)).toBeNull();
    const saved = snapshot();
    saveLibrarySession(saved);
    expect(loadLibrarySession(false)).toEqual(saved);
  });

  it('从外页带着初始筛选/关键词进入时忽略历史会话', () => {
    saveLibrarySession(snapshot());
    expect(loadLibrarySession(true)).toBeNull();
    expect(loadLibrarySession(false)?.selectedId).toBe('repo-42');
  });
});
