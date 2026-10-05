import { describe, expect, it } from 'vitest';
import {
  ABOUT_ME,
  INSTALL_COPY_HINT,
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

  it('直接装进独立环境并汇报（只有管理员权限 / 付费或登录 / 安全风险 / 很耗时才停下问）、不用 sudo、不跑 curl | bash、不读密钥、不替我登录发布', () => {
    expect(text).toContain('就直接装');
    expect(text).toContain('管理员权限');
    expect(text).not.toContain('我回「确认」');
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

describe('界面上的说明不能改动复制出去的正文', () => {
  it('类型仍是短名字，不带界面上那种长写法的解释', () => {
    const shortNames: [string, string][] = [
      ['skill', '技能'],
      ['mcp', 'MCP'],
      ['cli', '命令行'],
      ['app', '应用'],
      ['library', '代码库'],
      ['api', '接口'],
      ['service', '服务'],
      ['collection', '合集'],
    ];
    for (const [form, name] of shortNames) {
      const text = buildInstallPrompt({ fullName: 'a/tool', form });
      expect(text, form).toContain(`类型：${name}）`);
      expect(text, form).not.toMatch(/类型：[^）\n]*（/);
    }
    expect(buildInstallPrompt({ fullName: 'a/b' })).toContain('类型：工具');
    expect(buildInstallPrompt({ fullName: 'a/b', form: 'other' })).toContain('类型：工具');
  });

  it('正文仍说「没有风险就直接装」，界面上那句说明（INSTALL_COPY_HINT）讲的和它是同一件事', () => {
    const text = buildInstallPrompt({ fullName: 'a/skill', form: 'skill' });
    expect(text).toContain('就直接装，装完告诉我装了什么、装在哪、怎么卸载');
    expect(text).toContain('遇到这几种情况，先停下来问我');
    // 界面说明里讲的几件事，正文里都有：先查有没有、直接装进独立环境、要管理员密码 / 付费 / 登录先停下来问
    expect(INSTALL_COPY_HINT).toContain('先查你电脑上有没有');
    expect(INSTALL_COPY_HINT).toContain('直接装进独立文件夹');
    expect(INSTALL_COPY_HINT).toContain('要管理员密码、要付费或登录');
    expect(text).toContain('是不是已经装过');
    expect(text).toContain('依赖装进独立环境');
    expect(text).toContain('要管理员权限、要付费或登录账号');
  });
});
