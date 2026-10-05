import { describe, expect, it } from 'vitest';
import type { AutomationStatus } from './automation-view';
import {
  applyReadingOverrides,
  assembleWeekGroups,
  buildTodayLead,
  cleanHowToTry,
  cleanDescription,
  clipText,
  codexFallbackText,
  COPY_FAILED_NOTICE,
  copiedNotice,
  displayForYou,
  firstSentences,
  formatStars,
  greetingFor,
  itemsForReadingFilter,
  maturityLabel,
  maturityTagLabel,
  monthDay,
  pickReason,
  pickWhyLine,
  readmeBasisLabel,
  readmeTabLabel,
  readingGroupLabel,
  regenerateToast,
  relativeDays,
  splitRepoName,
} from './item-format';

describe('formatStars', () => {
  it('小于 1000 原样，0 也原样', () => {
    expect(formatStars(0)).toBe('0');
    expect(formatStars(1)).toBe('1');
    expect(formatStars(999)).toBe('999');
  });

  it('不到一万写整数，不再缩成 k', () => {
    expect(formatStars(1000)).toBe('1000');
    expect(formatStars(1200)).toBe('1200');
    expect(formatStars(1500)).toBe('1500');
    expect(formatStars(9999)).toBe('9999');
  });

  it('一万及以上写成万，一位小数，去掉末尾的 .0', () => {
    expect(formatStars(10000)).toBe('1 万');
    expect(formatStars(12000)).toBe('1.2 万');
    expect(formatStars(12345)).toBe('1.2 万');
    expect(formatStars(15000)).toBe('1.5 万');
    expect(formatStars(10500)).toBe('1.1 万');
    expect(formatStars(10050)).toBe('1 万');
    expect(formatStars(120000)).toBe('12 万');
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
  const now = new Date(2026, 9, 5, 12, 0, 0);

  it('去掉前导零，坏值返回破折号', () => {
    expect(monthDay('2026-10-03', now)).toBe('10 月 3 日');
    expect(monthDay('2026-01-09T00:00:00.000Z', now)).toBe('1 月 9 日');
    expect(monthDay('', now)).toBe('—');
    expect(monthDay(null, now)).toBe('—');
    expect(monthDay('10/03', now)).toBe('—');
    expect(monthDay('2026-13-40', now)).toBe('—');
  });

  it('只写日期、不写钟点：收藏时间是 GitHub 给的 UTC 时间，钟点和本机时间对不上', () => {
    expect(monthDay('2026-10-03T18:17:00Z', now)).toBe('10 月 3 日');
    expect(monthDay('2026-10-03T18:17:00Z', now)).not.toContain('18:17');
    expect(monthDay('2026-10-03T18:17:00.000+08:00', now)).toBe('10 月 3 日');
    expect(monthDay('2026-10-03 18:17:00', now)).toBe('10 月 3 日');
  });

  it('不是今年的带年份，今年的不带', () => {
    expect(monthDay('2025-10-03T18:17:00Z', now)).toBe('2025 年 10 月 3 日');
    expect(monthDay('2025-12-31', now)).toBe('2025 年 12 月 31 日');
    expect(monthDay('2026-01-01', now)).toBe('1 月 1 日');
    expect(monthDay('2027-01-01', now)).toBe('2027 年 1 月 1 日');
  });

  it('今年是哪一年看传进来的 now（Date、毫秒数、字符串都行）；读不出来就多写年份，不猜', () => {
    expect(monthDay('2026-10-03', now.getTime())).toBe('10 月 3 日');
    expect(monthDay('2026-10-03', '2026-12-01T12:00:00')).toBe('10 月 3 日');
    expect(monthDay('2026-10-03', '2027-01-02T12:00:00')).toBe('2026 年 10 月 3 日');
    expect(monthDay('2026-10-03', '不是日期')).toBe('2026 年 10 月 3 日');
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
  it('有推荐、没有推荐，以及负数和非数字', () => {
    expect(buildTodayLead(4, 3)).toBe('这周你收藏了 4 个新工具。先从下面 3 个你可能用得上的看起。');
    expect(buildTodayLead(0, 0)).toBe('这周你收藏了 0 个新工具。AI 整理好之后，这里会从还没看过的收藏里，优先挑和你关心的场景有关的。');
    expect(buildTodayLead(-3, -1)).toBe('这周你收藏了 0 个新工具。AI 整理好之后，这里会从还没看过的收藏里，优先挑和你关心的场景有关的。');
    expect(buildTodayLead(Number.NaN, Number.NaN)).toBe('这周你收藏了 0 个新工具。AI 整理好之后，这里会从还没看过的收藏里，优先挑和你关心的场景有关的。');
  });

  it('不再提全库的「等 AI 整理」数量', () => {
    expect(buildTodayLead(8, 3)).not.toContain('整理');
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

  it('复制出去的正文是改文案之前的原样（卡 32 / 33 都写明复制出去的话不改）：逐行对得上', () => {
    expect(codexFallbackText({
      fullName: 'fox/tool',
      htmlUrl: 'https://github.com/fox/tool',
      oneLiner: '做封面',
      howToTry: 'npm i tool',
    })).toBe([
      '请帮我试用这个工具：fox/tool（https://github.com/fox/tool）',
      '它是：做封面',
      '我想用它来：（请先问我想做什么）',
      'AI 整理的上手提示：npm i tool',
      '请先阅读 README，确认安装方式和使用前提，再带我一步步装好并跑通一个最小例子。不确定的地方先问我，不要编造安装步骤。',
    ].join('\n'));
    expect(codexFallbackText({ fullName: 'fox/tool' })).toBe([
      '请帮我试用这个工具：fox/tool（https://github.com/fox/tool）',
      '它是：（暂无说明）',
      '我想用它来：（请先问我想做什么）',
      '请先阅读 README，确认安装方式和使用前提，再带我一步步装好并跑通一个最小例子。不确定的地方先问我，不要编造安装步骤。',
    ].join('\n'));
  });
});

describe('名字、截断与今日列表', () => {
  it('splitRepoName 取名字首字母', () => {
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

  it('pickWhyLine 只在有 highlight 且和一句话不同时才给出', () => {
    expect(pickWhyLine({ oneLiner: '做封面', highlight: ' 批量做小红书封面 ' })).toBe('适合：批量做小红书封面');
    expect(pickWhyLine({ oneLiner: '做封面', highlight: '做封面' })).toBeNull();
    expect(pickWhyLine({ oneLiner: ' 做封面 ', highlight: '做封面' })).toBeNull();
    expect(pickWhyLine({ oneLiner: '做封面', highlight: '' })).toBeNull();
    expect(pickWhyLine({ oneLiner: '做封面', highlight: null })).toBeNull();
    expect(pickWhyLine({ oneLiner: '做封面' })).toBeNull();
    expect(pickWhyLine({ oneLiner: null, highlight: '做封面' })).toBe('适合：做封面');
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

describe('maturityLabel', () => {
  it('认识的英文换成中文，不分大小写', () => {
    expect(maturityLabel('usable')).toBe('可用');
    expect(maturityLabel(' Stable ')).toBe('稳定');
    expect(maturityLabel('EXPERIMENTAL')).toBe('实验中');
  });

  it('短词原样；整句话不显示（不截半句误导）', () => {
    expect(maturityLabel('较成熟')).toBe('较成熟');
    expect(maturityLabel('较高，具备详细文档、QC 自检流程、多种结构模板')).toBeNull();
    expect(maturityLabel('开源项目，提供安装脚本和文档，需自行部署。')).toBeNull();
  });

  it('空值返回 null', () => {
    expect(maturityLabel(null)).toBeNull();
    expect(maturityLabel('   ')).toBeNull();
  });
});

describe('cleanDescription', () => {
  it('去掉表情、:shortcode:、链接，空白折成一格', () => {
    expect(cleanDescription('🚀 抖音数据采集   工具 :hedgehog: https://example.com/a?b=1 好用')).toBe('抖音数据采集 工具 好用');
    expect(cleanDescription(':arrow_double_down: Dumb downloader')).toBe('Dumb downloader');
  });

  it('开头残留的分隔符也去掉；空值给空串', () => {
    expect(cleanDescription('| 一个工具')).toBe('一个工具');
    expect(cleanDescription(null)).toBe('');
    expect(cleanDescription('   ')).toBe('');
  });

  it('正常文字原样', () => {
    expect(cleanDescription('Python tool for converting files to Markdown.')).toBe('Python tool for converting files to Markdown.');
  });
});

describe('pickReason', () => {
  it('取所属的第一个场景写成推荐理由（统一叫「场景」，不叫「方向」）', () => {
    expect(pickReason(['小红书', '抖音·短视频'])).toBe('它被归在你关心的「小红书」场景，你还没看过');
    expect(pickReason(['  ', '电商'])).toBe('它被归在你关心的「电商」场景，你还没看过');
    expect(pickReason(['小红书'])).not.toContain('方向');
  });

  it('没有所属场景就不编', () => {
    expect(pickReason([])).toBeNull();
    expect(pickReason([' ', ''])).toBeNull();
  });
});


describe('成熟度的新写法', () => {
  it('可上生产改成可正式使用', () => {
    expect(maturityLabel('production')).toBe('可正式使用');
    expect(maturityLabel('Production')).toBe('可正式使用');
  });

  it('stale 翻译成久未更新，不再原样显示英文', () => {
    expect(maturityLabel('stale')).toBe('久未更新');
    expect(maturityLabel(' STALE ')).toBe('久未更新');
  });

  it('成熟度小标签带分组名；整句话仍然不显示', () => {
    expect(maturityTagLabel('usable')).toBe('成熟度：可用');
    expect(maturityTagLabel('mature')).toBe('成熟度：成熟');
    expect(maturityTagLabel('stale')).toBe('成熟度：久未更新');
    expect(maturityTagLabel('production')).toBe('成熟度：可正式使用');
    expect(maturityTagLabel('开源，提供安装脚本和文档，需自行部署。')).toBeNull();
    expect(maturityTagLabel(null)).toBeNull();
  });
});

describe('项目说明（README）的写法', () => {
  it('依据项目说明：只有简介和读过项目说明是两句，读过的那句带（README）', () => {
    expect(readmeBasisLabel('只有 GitHub 简介，没有 README：一个短介绍')).toBe('只依据 GitHub 简介，没读到项目说明，别当真');
    expect(readmeBasisLabel('  只有 GitHub 简介，没有 README：贴着开头')).toBe('只依据 GitHub 简介，没读到项目说明，别当真');
    expect(readmeBasisLabel('这是读过项目说明后写的中文说明')).toBe('依据项目说明（README）');
    expect(readmeBasisLabel('')).toBe('依据项目说明（README）');
    expect(readmeBasisLabel(null)).toBe('依据项目说明（README）');
    // 只认开头的前缀：句子中间出现不算
    expect(readmeBasisLabel('先说一句。只有 GitHub 简介，没有 README：不在开头')).toBe('依据项目说明（README）');
  });

  it('括号只在详情页第一次提到「项目说明」的地方出现一次：说明旁边那句带了，页签就不再带；那句没带（或没有说明）时页签带', () => {
    const readFromReadme = '读过项目说明后写的中文说明。';
    const fromIntroOnly = '只有 GitHub 简介，没有 README：只看了简介。';
    // 有中文说明、依据项目说明：说明旁边那句带括号，页签只写「项目说明」
    expect(readmeTabLabel(true, readFromReadme)).toBe('项目说明');
    // 没有中文说明：页面上没有「依据…」那句，页签是第一次出现
    expect(readmeTabLabel(false, null)).toBe('项目说明（README）');
    expect(readmeTabLabel(false, readFromReadme)).toBe('项目说明（README）');
    // 说明只依据简介：那句话是审定的整句、不带括号，页签补上
    expect(readmeTabLabel(true, fromIntroOnly)).toBe('项目说明（README）');

    const mentions = (hasCard: boolean, summary: string) => {
      const basis = hasCard ? readmeBasisLabel(summary) : '';
      return [basis, readmeTabLabel(hasCard, summary)].join('|').split('项目说明（README）').length - 1;
    };
    expect(mentions(true, readFromReadme)).toBe(1);
    expect(mentions(false, readFromReadme)).toBe(1);
    expect(mentions(true, fromIntroOnly)).toBe(1);
  });
});

describe('灵感里的 Fox 显示成你', () => {
  it('单独的 Fox 换成你，大小写敏感，可以出现多次', () => {
    expect(displayForYou('给 Fox 的灵感')).toBe('给你的灵感');
    expect(displayForYou('Fox 先看，再给 Fox 用')).toBe('你先看，再给你用');
    expect(displayForYou('fox 不换')).toBe('fox 不换');
    expect(displayForYou('FOX 不换')).toBe('FOX 不换');
    expect(displayForYou('  ')).toBe('');
    expect(displayForYou(null)).toBe('');
    expect(displayForYou(undefined)).toBe('');
  });

  it('词里面带 Fox 的不动（Firefox 插件、Foxmail 之类）', () => {
    expect(displayForYou('Firefox 插件，给 Fox 用')).toBe('Firefox 插件，给你用');
    expect(displayForYou('Foxmail 的替代品')).toBe('Foxmail 的替代品');
    expect(displayForYou('FoxitReader 打开 PDF')).toBe('FoxitReader 打开 PDF');
  });

  it('换成你以后，中文和你之间不留空格；英文单词旁的空格保留', () => {
    expect(displayForYou('适合 Fox 做小红书封面')).toBe('适合你做小红书封面');
    expect(displayForYou('Notion 同步，Fox 可以直接用')).toBe('Notion 同步，你可以直接用');
    expect(displayForYou('Fox, try it')).toBe('你, try it');
  });
});

describe('复制给 Codex 之后的提示（按钮上的字见详情页和发现页卡片的渲染测试）', () => {
  it('复制成功的提示：装软件的那种说清装在哪、怎么卸载；只评估的说明不会安装', () => {
    const install = copiedNotice('install');
    expect(install).toBe('已复制。粘到 Codex 后，它会先查你电脑上有没有，没有就装进独立文件夹，装完告诉你装在哪、怎么卸载。');
    expect(install).toContain('装进独立文件夹');
    expect(install).toContain('怎么卸载');
    const evaluate = copiedNotice('evaluate');
    expect(evaluate).toBe('已复制。它只会读资料、给你结论，不会安装。');
    expect(evaluate).toContain('不会安装');
    expect(evaluate).not.toContain('卸载');
  });

  it('让 Codex 去查别人怎么用不装东西，提示不能说「会装进独立文件夹」', () => {
    const community = copiedNotice('community');
    expect(community).toBe('已复制，去 Codex 里粘贴');
    expect(community).not.toContain('装进');
    expect(community).not.toContain('卸载');
  });

  it('复制不了的提示是桌面应用的说法，不提「浏览器」', () => {
    expect(COPY_FAILED_NOTICE).toBe('没能自动复制，请手动选中后按 ⌘C');
    expect(COPY_FAILED_NOTICE).not.toContain('浏览器');
  });
});

describe('regenerateToast：点「不对，请 AI 重写」之后的提示', () => {
  function status(overrides: Partial<AutomationStatus> = {}): AutomationStatus {
    return {
      enabled: true,
      dailyBudgetTokens: 100_000,
      paused: false,
      pauseReason: null,
      tokens24h: 0,
      budgetRemaining: 100_000,
      queued: 0,
      running: 0,
      failed: 0,
      providerConfigured: true,
      model: 'gpt-x',
      ...overrides,
    };
  }

  it('自动整理没开：说清已记下但不会重写，开启后才会重写，保留「去开启」', () => {
    expect(regenerateToast(status({ enabled: false, dailyBudgetTokens: 0, budgetRemaining: 0 }))).toEqual({
      title: '已记下，但自动整理没开，不会重写。',
      sub: '开启后才会重写。',
      actionLabel: '去开启',
    });
  });

  it('没开的时候不许说「会在后台重新整理」「好了会自动更新」（那是不会兑现的承诺）', () => {
    const note = regenerateToast(status({ enabled: false }));
    expect(`${note.title}${note.sub}`).not.toContain('后台重新整理');
    expect(`${note.title}${note.sub}`).not.toContain('自动更新');
  });

  it('没配置服务 / 暂停 / 一次性全部整理，开关关着也各说各的，不套用「没开」那句', () => {
    expect(regenerateToast(status({ providerConfigured: false, enabled: false }))).toEqual({
      title: '已加入整理队列',
      sub: '还没配置 AI 服务，配置好才会重新整理',
      actionLabel: '去设置 AI',
    });
    const paused = regenerateToast(status({ paused: true, enabled: false }));
    expect(paused.title).toBe('已加入整理队列');
    expect(paused.sub).toContain('暂停');
    expect(paused.actionLabel).toBe('去处理');
    const fullScan = regenerateToast(status({ unlimited: true, enabled: false }));
    expect(fullScan.title).toBe('已加入整理队列');
    expect(fullScan.sub).toContain('全量整理');
    expect(fullScan.actionLabel).toBeNull();
  });

  it('开着的时候照原来的提示：读不到状态只说已加入；额度用完说明天；正常在跑说会在后台重新整理', () => {
    expect(regenerateToast(null)).toEqual({ title: '已加入整理队列', actionLabel: null });
    expect(regenerateToast(status({ budgetRemaining: 0 })).sub).toContain('明天');
    expect(regenerateToast(status({ queued: 2 }))).toEqual({
      title: '已加入整理队列',
      sub: '会在后台重新整理，好了会自动更新',
      actionLabel: null,
    });
  });
});
