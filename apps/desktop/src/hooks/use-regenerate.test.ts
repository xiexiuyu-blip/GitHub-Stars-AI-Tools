import { describe, expect, it } from 'vitest';
import SOURCE from '@/hooks/use-regenerate.ts?raw';

describe('重新整理这一张：确认之后才调用', () => {
  it('命令固定带 confirmed: true，不再读自动整理的额度', () => {
    expect(SOURCE).toContain('regenerateLibraryCard(id, true)');
    expect(SOURCE).not.toContain('regenerateLibraryCard(id, false)');
    expect(SOURCE).not.toContain('getAiAutomation');
  });

  it('失败才按暂停或没配置决定按钮，成功提示不带按钮', () => {
    expect(SOURCE).toContain('regenerateFailureNote(');
    expect(SOURCE).toContain('regenerateStartedNote()');
    expect(SOURCE).toContain('failure.actionLabel && onOpenToolbox');
    const started = SOURCE.slice(SOURCE.indexOf('regenerateStartedNote()'));
    expect(started).toContain('toast.show({ title: note.title, sub: note.sub })');
    expect(started).not.toContain('actions');
  });
});
