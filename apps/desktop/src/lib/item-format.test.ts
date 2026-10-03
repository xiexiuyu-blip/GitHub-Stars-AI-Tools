import { describe, expect, it } from 'vitest';
import {
  applyReadingOverrides,
  assembleWeekGroups,
  buildTodayLead,
  cleanHowToTry,
  clipText,
  codexFallbackText,
  firstSentences,
  formatStars,
  greetingFor,
  itemsForReadingFilter,
  monthDay,
  pickWhyLine,
  readingGroupLabel,
  relativeDays,
  splitRepoName,
} from './item-format';

describe('formatStars', () => {
  it('小于 1000 原样，0 也原样', () => {
    expect(formatStars(0)).toBe('0');
    expect(formatStars(1)).toBe('1');
    expect(formatStars(999)).toBe('999');
  });

  it('1000 与 1200 写成 k 并去掉 .0', () => {
    expect(formatStars(1000)).toBe('1k');
    expect(formatStars(1200)).toBe('1.2k');
    expect(formatStars(1500)).toBe('1.5k');
  });

  it('12000 去掉 .0，12345 保留一位小数', () => {
    expect(formatStars(12000)).toBe('12k');
    expect(formatStars(12345)).toBe('12.3k');
    expect(formatStars(10000)).toBe('10k');
  });

  it('负数、NaN、无穷大返回破折号', () => {
    expect(formatStars(-1)).toBe('—');
    expect(formatStars(Number.NaN)).toBe('—');
    expect(formatStars(Number.POSITIVE_INFINITY)).toBe('—');
  });
});

describe('relativeDays', () => {
  it('空值、空白和坏日期返回破折号', () => {
    expect(relativeDays('', new Date('2026-10-03T00:00:00.000Z'))).toBe('—');
    expect(relativeDays('   ', new Date('2026-10-03T00:00:00.000Z'))).toBe('—');
    expect(relativeDays(null, new Date('2026-10-03T00:00:00.000Z'))).toBe('—');
    expect(relativeDays('不是日期', new Date('2026-10-03T00:00:00.000Z'))).toBe('—');
    expect(relativeDays('2026-10-03T00:00:00.000Z', '也不是日期')).toBe('—');
  });

  it('同一天、未来（负数天）算今天，刚好一天算昨天', () => {
    const now = new Date('2026-10-03T00:00:00.000Z');
    expect(relativeDays('2026-10-03T00:00:00.000Z', now)).toBe('今天');
    expect(relativeDays('2026-10-04T00:00:00.000Z', now)).toBe('今天');
    expect(relativeDays('2026-10-02T00:00:00.000Z', now)).toBe('昨天');
  });

  it('0 点附近：不足 12 小时算今天，满 12 小时四舍五入为昨天', () => {
    expect(relativeDays('2026-10-03T00:00:00.000Z', new Date('2026-10-03T11:00:00.000Z'))).toBe('今天');
    expect(relativeDays('2026-10-03T00:00:00.000Z', new Date('2026-10-03T12:00:00.000Z'))).toBe('昨天');
  });

  it('整点相差 24 小时是昨天', () => {
    expect(relativeDays('2026-10-02T15:00:00.000Z', new Date('2026-10-03T15:00:00.000Z'))).toBe('昨天');
  });

  it('29 天、30 天、364 天、365 天的档位', () => {
    const start = new Date('2026-01-01T00:00:00.000Z');
    const at = (days: number) => new Date(start.getTime() + days * 86_400_000);
    expect(relativeDays(start.toISOString(), at(29))).toBe('29 天前');
    expect(relativeDays(start.toISOString(), at(30))).toBe('1 个月前');
    expect(relativeDays(start.toISOString(), at(364))).toBe('12 个月前');
    expect(relativeDays(start.toISOString(), at(365))).toBe('1 年前');
  });
});

describe('monthDay', () => {
  it('去掉前导零，坏值返回破折号', () => {
    expect(monthDay('2026-10-03')).toBe('10/3');
    expect(monthDay('2026-01-09T00:00:00.000Z')).toBe('1/9');
    expect(monthDay('')).toBe('—');
    expect(monthDay(null)).toBe('—');
    expect(monthDay('10/03')).toBe('—');
    expect(monthDay('2026-13-40')).toBe('—');
  });
});

describe('firstSentences', () => {
  it('按中文句号取前 n 句并补回句号', () => {
    expect(firstSentences('你好。世界。再见。', 2)).toBe('你好。世界。');
    expect(firstSentences('你好。世界。再见。', 5)).toBe('你好。世界。再见。');
    expect(firstSentences('没有句号的一段', 1)).toBe('没有句号的一段。');
    expect(firstSentences('Hello. World.', 1)).toBe('Hello. World.。');
    expect(firstSentences('', 3)).toBe('');
    expect(firstSentences(null, 3)).toBe('');
    expect(firstSentences('你好。', 0)).toBe('');
    expect(firstSentences('你好。', -1)).toBe('');
  });
});

describe('cleanHowToTry', () => {
  it('空和占位话返回空串，其余原样', () => {
    expect(cleanHowToTry('')).toBe('');
    expect(cleanHowToTry('   ')).toBe('');
    expect(cleanHowToTry(null)).toBe('');
    expect(cleanHowToTry('摘录中未提供安装步骤')).toBe('');
    expect(cleanHowToTry('文档里未提供命令')).toBe('');
    expect(cleanHowToTry('没有提供安装方式')).toBe('');
    expect(cleanHowToTry('步骤未展示')).toBe('');
    expect(cleanHowToTry('  npm i tool  ')).toBe('  npm i tool  ');
  });
});

describe('readingGroupLabel', () => {
  it('五档中文', () => {
    expect(readingGroupLabel('unseen')).toBe('没看');
    expect(readingGroupLabel('want')).toBe('想试');
    expect(readingGroupLabel('tried')).toBe('试过');
    expect(readingGroupLabel('in_use')).toBe('在用');
    expect(readingGroupLabel('dropped')).toBe('不要了');
  });
});

describe('greetingFor', () => {
  it('0 点、整点边界、负数和非数字', () => {
    expect(greetingFor(0)).toBe('晚上好');
    expect(greetingFor(4)).toBe('晚上好');
    expect(greetingFor(5)).toBe('早上好');
    expect(greetingFor(10)).toBe('早上好');
    expect(greetingFor(10.9)).toBe('早上好');
    expect(greetingFor(11)).toBe('中午好');
    expect(greetingFor(13)).toBe('中午好');
    expect(greetingFor(14)).toBe('下午好');
    expect(greetingFor(17)).toBe('下午好');
    expect(greetingFor(18)).toBe('晚上好');
    expect(greetingFor(23)).toBe('晚上好');
    expect(greetingFor(24)).toBe('晚上好');
    expect(greetingFor(-1)).toBe('晚上好');
    expect(greetingFor(Number.NaN)).toBe('晚上好');
  });
});

describe('buildTodayLead', () => {
  it('有待整理、有推荐、都没有，以及负数', () => {
    expect(buildTodayLead(4, 2, 3)).toBe('这周你收藏了 4 个新工具，其中 2 个正在等 AI 整理。先从下面 3 个你可能用得上的看起。');
    expect(buildTodayLead(4, 0, 3)).toBe('这周你收藏了 4 个新工具。先从下面 3 个你可能用得上的看起。');
    expect(buildTodayLead(0, 0, 0)).toBe('这周你收藏了 0 个新工具。AI 整理好之后，这里会按你常做的事挑出值得先看的。');
    expect(buildTodayLead(-3, Number.NaN, -1)).toBe('这周你收藏了 0 个新工具。AI 整理好之后，这里会按你常做的事挑出值得先看的。');
  });
});

describe('codexFallbackText', () => {
  it('带上手提示，占位的怎么试不写进去', () => {
    const withHow = codexFallbackText({
      fullName: 'fox/tool',
      htmlUrl: 'https://github.com/fox/tool',
      oneLiner: '做封面',
      howToTry: 'npm i tool',
    });
    expect(withHow).toContain('请帮我试用这个工具：fox/tool（https://github.com/fox/tool）');
    expect(withHow).toContain('它是：做封面');
    expect(withHow).toContain('AI 整理的上手提示：npm i tool');
    expect(withHow).toContain('不要编造安装步骤');

    const withoutHow = codexFallbackText({
      fullName: 'fox/tool',
      description: '英文描述',
      howToTry: '未提供',
    });
    expect(withoutHow).toContain('它是：英文描述');
    expect(withoutHow).not.toContain('上手提示');
    expect(withoutHow).toContain('https://github.com/fox/tool');
  });
});

describe('仓库名、截断与今日列表', () => {
  it('splitRepoName 取仓库名首字母', () => {
    expect(splitRepoName('Fox/Tool')).toEqual({ owner: 'Fox', repo: 'Tool', initial: 'T' });
    expect(splitRepoName('仅名字')).toEqual({ owner: '', repo: '仅名字', initial: '仅' });
    expect(splitRepoName('   ')).toEqual({ owner: '', repo: '', initial: '?' });
  });

  it('clipText 按字符截断，中文不被切成半个', () => {
    expect(clipText('abc', 3)).toBe('abc');
    expect(clipText('abcd', 3)).toBe('abc…');
    expect(clipText('中文能力说明', 2)).toBe('中文…');
    expect(clipText('中文', 0)).toBe('');
  });

  it('pickWhyLine 只用一句话', () => {
    expect(pickWhyLine(' 做封面 ')).toBe('适合：做封面');
    expect(pickWhyLine('')).toBeNull();
    expect(pickWhyLine(null)).toBeNull();
  });

  it('assembleWeekGroups 隐藏已标状态的推荐，并去掉重复和不要了', () => {
    const picks = [
      { id: 'a', readingGroup: 'unseen' },
      { id: 'b', readingGroup: 'want' },
      { id: 'a', readingGroup: 'unseen' },
    ];
    const weekNew = [
      { id: 'a', readingGroup: 'unseen' },
      { id: 'c', readingGroup: 'want' },
      { id: 'd', readingGroup: 'dropped' },
      { id: 'e', readingGroup: 'unseen' },
    ];
    expect(assembleWeekGroups(picks, weekNew)).toEqual({
      picks: [{ id: 'a', readingGroup: 'unseen' }],
      rest: [
        { id: 'c', readingGroup: 'want' },
        { id: 'e', readingGroup: 'unseen' },
      ],
    });
  });

  it('applyReadingOverrides 只改被标过的条目', () => {
    const items = [
      { id: 'a', readingGroup: 'unseen', name: 'A' },
      { id: 'b', readingGroup: 'want', name: 'B' },
    ];
    expect(applyReadingOverrides(items, { a: 'dropped' })).toEqual([
      { id: 'a', readingGroup: 'dropped', name: 'A' },
      { id: 'b', readingGroup: 'want', name: 'B' },
    ]);
    expect(applyReadingOverrides(items, { b: 'want' })[1]).toBe(items[1]);
  });

  it('itemsForReadingFilter 只留下指定状态', () => {
    const items = [
      { id: 'a', readingGroup: 'want' },
      { id: 'b', readingGroup: 'dropped' },
      { id: 'c', readingGroup: 'want' },
    ];
    expect(itemsForReadingFilter(items, 'want').map((item) => item.id)).toEqual(['a', 'c']);
    expect(itemsForReadingFilter(items, 'in_use')).toEqual([]);
  });
});
