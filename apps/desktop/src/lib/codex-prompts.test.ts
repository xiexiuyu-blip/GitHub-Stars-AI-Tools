import { describe, expect, it } from 'vitest';
import {
  ABOUT_ME,
  buildCommunityPrompt,
  buildInstallPrompt,
  installTarget,
  WORK_RULES,
  workRules,
} from './codex-prompts';

describe('workRules', () => {
  it('固定七条；编号别处会引用，数量变了要同时改引用', () => {
    expect(WORK_RULES).toHaveLength(7);
  });

  it('不传编号是全部；传了就按给定顺序重新从 1 编号', () => {
    expect(workRules().split('\n')).toHaveLength(7);
    const picked = workRules([4, 5]).split('\n');
    expect(picked).toHaveLength(2);
    expect(picked[0].startsWith('1. 不用 sudo')).toBe(true);
    expect(picked[1].startsWith('2. 第三方 README')).toBe(true);
  });

  it('越界的编号被忽略', () => {
    expect(workRules([0, 8, 99])).toBe('');
  });
});

describe('installTarget', () => {
  it('Skill 装到 ~/.agents/skills，并纠正 README 里常写的 ~/.claude/skills', () => {
    const text = installTarget('skill', 'guizang-social-card-skill');
    expect(text).toContain('~/.agents/skills/guizang-social-card-skill');
    expect(text).toContain('~/.claude/skills');
  });

  it('各种形态都有说法，不认识的走默认', () => {
    expect(installTarget('mcp', 'x')).toContain('codex mcp add');
    expect(installTarget('app', 'x')).toContain('应用程序');
    expect(installTarget('cli', 'x')).toContain('独立环境');
    expect(installTarget('library', 'x')).toContain('venv');
    expect(installTarget('service', 'x')).toContain('Docker');
    expect(installTarget(null, 'x')).toContain('独立的新目录');
    expect(installTarget('weird', 'x')).toContain('独立的新目录');
  });
});

describe('buildInstallPrompt', () => {
  const text = buildInstallPrompt({
    fullName: 'op7418/guizang-social-card-skill',
    htmlUrl: 'https://github.com/op7418/guizang-social-card-skill',
    form: 'skill',
    scenarioName: '小红书',
  });

  it('带项目名、链接、类型、我想做什么、关于我', () => {
    expect(text).toContain('「guizang-social-card-skill」');
    expect(text).toContain('https://github.com/op7418/guizang-social-card-skill');
    expect(text).toContain('类型：技能');
    expect(text).toContain('「小红书」');
    expect(text).toContain(ABOUT_ME);
  });

  it('先给计划等「确认」、不用 sudo、不跑 curl | bash、不读密钥、不替我登录发布', () => {
    expect(text).toContain('我回「确认」你再动手');
    expect(text).toContain('不用 sudo');
    expect(text).toContain('curl | bash');
    expect(text).toContain('不要读取也不要打印');
    expect(text).toContain('不要登录我的账号');
  });

  it('说清装到哪、怎么卸载、失败两次就停、最后五行汇报', () => {
    expect(text).toContain('~/.agents/skills/guizang-social-card-skill');
    expect(text).toContain('怎么卸载');
    expect(text).toContain('失败 2 次');
    expect(text).toContain('5 行汇报');
  });

  it('没有链接就用 fullName 拼；没有形态写「工具」；没有场景写「我手头的事」', () => {
    const bare = buildInstallPrompt({ fullName: 'a/b' });
    expect(bare).toContain('https://github.com/a/b');
    expect(bare).toContain('类型：工具');
    expect(bare).toContain('我手头的事');
  });
});

describe('buildCommunityPrompt', () => {
  const text = buildCommunityPrompt(
    { fullName: 'harry0703/MoneyPrinterTurbo', htmlUrl: 'https://github.com/harry0703/MoneyPrinterTurbo', scenarioName: '抖音·短视频' },
    new Date('2026-10-03T08:00:00'),
  );

  it('带项目名、链接、今天的日期、场景', () => {
    expect(text).toContain('「MoneyPrinterTurbo」');
    expect(text).toContain('https://github.com/harry0703/MoneyPrinterTurbo');
    expect(text).toContain('今天是 2026-10-03');
    expect(text).toContain('「抖音·短视频」');
  });

  it('要求标日期和作者类型、分清软文、查不到就直说', () => {
    expect(text).toContain('链接｜日期｜作者类型');
    expect(text).toContain('疑似软文');
    expect(text).toContain('2 个独立来源');
    expect(text).toContain('抓不到就直说');
    expect(text).toContain('不要拿搜索摘要猜');
  });

  it('排除同名项目、给结论', () => {
    expect(text).toContain('排除同名的其他项目');
    expect(text).toContain('值得试 / 观望 / 别试');
  });
});
