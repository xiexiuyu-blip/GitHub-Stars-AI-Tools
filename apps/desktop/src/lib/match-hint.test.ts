import { describe, expect, it } from 'vitest';
import { MATCH_STAR_HINTS, MATCH_STAR_MAX, matchReasonsLine, normalizeMatchStars } from './match-hint';

describe('匹配度档位', () => {
  it('只认 1、2、3、4；其它一律没有档位', () => {
    expect(MATCH_STAR_MAX).toBe(4);
    expect(normalizeMatchStars(1)).toBe(1);
    expect(normalizeMatchStars(2)).toBe(2);
    expect(normalizeMatchStars(3)).toBe(3);
    expect(normalizeMatchStars(4)).toBe(4);
    for (const value of [0, 5, 2.5, -1, Number.NaN, '3', null, undefined]) {
      expect(normalizeMatchStars(value), String(value)).toBeNull();
    }
  });

  it('四档悬停文案固定', () => {
    expect(Object.keys(MATCH_STAR_HINTS)).toEqual(['1', '2', '3', '4']);
    expect(MATCH_STAR_HINTS).toEqual({
      4: '匹配度 4 / 4：搜的词主要出现在项目名里',
      3: '匹配度 3 / 4：搜的词主要出现在中文一句话说明里',
      2: '匹配度 2 / 4：搜的词主要出现在中文摘要或英文简介里',
      1: '匹配度 1 / 4：只是沾边，多在话题或关键词里',
    });
  });

  it('命中原因只留有效且不重复的字符串，最多四个', () => {
    expect(matchReasonsLine(['项目名', '中文说明'])).toBe('命中：项目名 / 中文说明');
    expect(matchReasonsLine([])).toBeNull();
    expect(matchReasonsLine(null)).toBeNull();
    expect(matchReasonsLine(undefined)).toBeNull();
    expect(matchReasonsLine(['', '   ', 1, '  项目名  ', '项目名', '中文说明'])).toBe('命中：项目名 / 中文说明');
    expect(matchReasonsLine(['项目名', '中文说明', '英文简介', '话题', '多出来'])).toBe(
      '命中：项目名 / 中文说明 / 英文简介 / 话题',
    );
  });
});
