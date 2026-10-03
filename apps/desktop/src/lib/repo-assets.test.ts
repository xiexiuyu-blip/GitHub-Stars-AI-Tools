import { describe, expect, it } from 'vitest';
import {
  avatarUrl,
  buildCommunityPrompt,
  buildInstallPrompt,
  communityLinks,
  coverGradient,
  extractInstallSteps,
  extractPreviewImages,
  hasDownloadHint,
  hueOf,
  languageColor,
  ownerOf,
  releasesUrl,
  repoNameOf,
  resolveAssetUrl,
  scenarioHue,
  searchTerm,
} from './repo-assets';

describe('名字与头像', () => {
  it('拆所有者和仓库名', () => {
    expect(ownerOf('harry0703/MoneyPrinterTurbo')).toBe('harry0703');
    expect(repoNameOf('harry0703/MoneyPrinterTurbo')).toBe('MoneyPrinterTurbo');
    expect(ownerOf('noslash')).toBe('');
    expect(repoNameOf('noslash')).toBe('noslash');
  });

  it('头像地址只给合法的所有者名，尺寸夹在 16–460', () => {
    expect(avatarUrl('op7418/guizang-social-card-skill', 96)).toBe('https://github.com/op7418.png?size=96');
    expect(avatarUrl('op7418/x', 4)).toBe('https://github.com/op7418.png?size=16');
    expect(avatarUrl('op7418/x', 9999)).toBe('https://github.com/op7418.png?size=460');
    expect(avatarUrl('noslash')).toBeNull();
    expect(avatarUrl('bad owner/x')).toBeNull();
  });
});

describe('颜色', () => {
  it('同一段文字永远同一个色相，且在 0–359', () => {
    const hue = hueOf('harry0703');
    expect(hue).toBe(hueOf('HARRY0703'));
    expect(hue).toBeGreaterThanOrEqual(0);
    expect(hue).toBeLessThan(360);
    expect(hueOf('a')).not.toBe(hueOf('b'));
  });

  it('封面底色是渐变，由名字决定', () => {
    expect(coverGradient('x')).toContain('linear-gradient');
    expect(coverGradient('x')).toBe(coverGradient('X'));
  });

  it('语言颜色不分大小写，不认识的语言没有颜色', () => {
    expect(languageColor('TypeScript')).toBe('#3178c6');
    expect(languageColor(' python ')).toBe('#3572a5');
    expect(languageColor('Brainfuck')).toBeNull();
    expect(languageColor(null)).toBeNull();
  });

  it('场景色相：slug 和场景 id 都认，十三个互不相同，不认识的按名字稳定取', () => {
    expect(scenarioHue('xhs')).toBe(scenarioHue('scn-xhs'));
    const hues = ['xhs', 'wx', 'dy', 'img', 'vid', 'ppt', 'wr', 'res', 'data', 'kb', 'auto', 'ec', 'seo'].map(scenarioHue);
    expect(new Set(hues).size).toBe(13);
    expect(scenarioHue('custom-one')).toBe(scenarioHue('custom-one'));
    expect(scenarioHue(null)).toBe(scenarioHue(undefined));
  });

  it('下载页地址', () => {
    expect(releasesUrl('a/b')).toBe('https://github.com/a/b/releases/latest');
  });
});

describe('resolveAssetUrl', () => {
  it('相对路径拼到 raw.githubusercontent.com，去掉 ./ 和 ..', () => {
    expect(resolveAssetUrl('screenshot/main page.png', 'a/b')).toBe('https://raw.githubusercontent.com/a/b/HEAD/screenshot/main%20page.png');
    expect(resolveAssetUrl('./docs/a.png', 'a/b')).toBe('https://raw.githubusercontent.com/a/b/HEAD/docs/a.png');
    expect(resolveAssetUrl('/docs/a.png', 'a/b')).toBe('https://raw.githubusercontent.com/a/b/HEAD/docs/a.png');
    expect(resolveAssetUrl('../x/a.png', 'a/b')).toBe('https://raw.githubusercontent.com/a/b/HEAD/x/a.png');
  });

  it('绝对地址原样；协议相对补 https；github blob 改成 raw；data 和锚点丢掉', () => {
    expect(resolveAssetUrl('https://x.com/a.png', 'a/b')).toBe('https://x.com/a.png');
    expect(resolveAssetUrl('//cdn.x.com/a.png', 'a/b')).toBe('https://cdn.x.com/a.png');
    expect(resolveAssetUrl('https://github.com/o/r/blob/main/img/a.png?raw=true', 'a/b')).toBe('https://raw.githubusercontent.com/o/r/main/img/a.png');
    expect(resolveAssetUrl('data:image/png;base64,AAAA', 'a/b')).toBeNull();
    expect(resolveAssetUrl('#top', 'a/b')).toBeNull();
    expect(resolveAssetUrl('mailto:a@b.c', 'a/b')).toBeNull();
    expect(resolveAssetUrl('  ', 'a/b')).toBeNull();
  });
});

describe('extractPreviewImages', () => {
  const readme = [
    '[![Stars](https://img.shields.io/github/stars/a/b)](https://github.com/a/b)',
    '![logo](assets/logo.png)',
    '![主界面](screenshot/main_page.png)',
    '<p align="center"><img src="docs/webui.jpg" alt="Web 界面" width="600"></p>',
    '![](https://raw.githubusercontent.com/andreasbm/readme/master/assets/lines/rainbow.png)',
    '![流程](docs/flow.svg)',
    '![重复](screenshot/main_page.png)',
    '![动图](https://github.com/user-attachments/assets/414b8838-6731-45d4-a815-6e3c0aa1fe52)',
  ].join('\n');

  it('按出现顺序取截图，去掉徽章、分隔线、svg、重复；logo 排在没有别的图时才用', () => {
    const images = extractPreviewImages(readme, 'a/b');
    expect(images.map((item) => item.url)).toEqual([
      'https://raw.githubusercontent.com/a/b/HEAD/screenshot/main_page.png',
      'https://raw.githubusercontent.com/a/b/HEAD/docs/webui.jpg',
      'https://github.com/user-attachments/assets/414b8838-6731-45d4-a815-6e3c0aa1fe52',
    ]);
    expect(images[0].alt).toBe('主界面');
    expect(images[1].alt).toBe('Web 界面');
  });

  it('限制数量；limit 为 0 返回空', () => {
    expect(extractPreviewImages(readme, 'a/b', 2)).toHaveLength(2);
    expect(extractPreviewImages(readme, 'a/b', 0)).toEqual([]);
  });

  it('只有 logo 时退回 logo；什么图都没有返回空', () => {
    expect(extractPreviewImages('![logo](assets/logo.png)', 'a/b').map((item) => item.url)).toEqual(['https://raw.githubusercontent.com/a/b/HEAD/assets/logo.png']);
    expect(extractPreviewImages('只有文字', 'a/b')).toEqual([]);
    expect(extractPreviewImages('', 'a/b')).toEqual([]);
  });
});

describe('extractInstallSteps', () => {
  const moneyPrinter = [
    '# MoneyPrinterTurbo',
    '简介',
    '### ① 启动Docker',
    '```bash',
    'cd MoneyPrinterTurbo',
    'docker compose -f docker-compose.release.yml up',
    '```',
    '### ② 启动Web界面 🌐',
    '```bash',
    'uv run streamlit run ./webui/Main.py --browser.gatherUsageStats=False',
    '```',
    '### 常见问题',
    '```bash',
    'echo 不该出现',
    '```',
  ].join('\n');

  it('取「安装 / 启动」标题下面的命令块，标题去掉序号和装饰', () => {
    const steps = extractInstallSteps(moneyPrinter);
    expect(steps).toHaveLength(2);
    expect(steps[0]).toEqual({ title: '启动Docker', command: 'cd MoneyPrinterTurbo\ndocker compose -f docker-compose.release.yml up', kind: 'docker' });
    expect(steps[1].kind).toBe('python');
    expect(steps[1].command).toContain('uv run streamlit');
  });

  it('一行命令安装（npx skills add）认作 npm 类', () => {
    const md = '## 方式一:一行命令安装(推荐)\n```bash\nnpx skills add https://github.com/op7418/guizang-social-card-skill --skill guizang-social-card-skill\n```';
    expect(extractInstallSteps(md)).toEqual([
      { title: '方式一:一行命令安装(推荐)', command: 'npx skills add https://github.com/op7418/guizang-social-card-skill --skill guizang-social-card-skill', kind: 'npm' },
    ]);
  });

  it('curl 一键安装脚本认作 curl 类，去掉 $ 提示符', () => {
    const md = '## 安装 PandaWiki\n```\n$ bash -c "$(curl -fsSLk https://release.baizhi.cloud/panda-wiki/manager.sh)"\n```';
    const steps = extractInstallSteps(md);
    expect(steps).toHaveLength(1);
    expect(steps[0].command.startsWith('bash -c')).toBe(true);
  });

  it('没有安装标题时，退回全文里最像安装命令的块；配置文件块不算', () => {
    const md = ['## 配置', '```json', '{"a": 1}', '```', '## 其他', '```', 'pip install foo', '```'].join('\n');
    const steps = extractInstallSteps(md);
    expect(steps).toEqual([{ title: '安装', command: 'pip install foo', kind: 'python' }]);
  });

  it('安装标题下的 JSON / yaml 配置块不当成命令', () => {
    const md = '## Installation\n```json\n{"mcpServers": {}}\n```';
    expect(extractInstallSteps(md)).toEqual([]);
  });

  it('太长的块只留前 10 行；limit 生效；相同命令不重复；没有命令返回空', () => {
    const long = Array.from({ length: 14 }, (_, i) => `echo ${i}`).join('\n');
    const steps = extractInstallSteps(`## Install\n\`\`\`bash\n${long}\n\`\`\``);
    expect(steps[0].command.split('\n')).toHaveLength(11);
    expect(steps[0].command.endsWith('…')).toBe(true);
    const many = ['a', 'b', 'c', 'd'].map((x) => `## Install ${x}\n\`\`\`bash\nnpm i ${x}\n\`\`\``).join('\n');
    expect(extractInstallSteps(many, 2)).toHaveLength(2);
    expect(extractInstallSteps('## Install\n```bash\nnpm i x\n```\n## Setup\n```bash\nnpm i x\n```')).toHaveLength(1);
    expect(extractInstallSteps('只有文字')).toEqual([]);
    expect(extractInstallSteps('')).toEqual([]);
  });

  it('下载安装包的迹象', () => {
    expect(hasDownloadHint('到 Releases 下载 .dmg')).toBe(true);
    expect(hasDownloadHint('请下载安装包')).toBe(true);
    expect(hasDownloadHint('npm i foo')).toBe(false);
  });
});

describe('去别处看别人怎么用', () => {
  it('搜索词：名字够长够具体就只用仓库名，太短或太通用就带上所有者', () => {
    expect(searchTerm('harry0703/MoneyPrinterTurbo')).toBe('MoneyPrinterTurbo');
    expect(searchTerm('MengTo/Skills')).toBe('MengTo Skills');
    expect(searchTerm('ChatCut-Inc/agent-plugin')).toBe('ChatCut-Inc agent-plugin');
    expect(searchTerm('x/ab')).toBe('x ab');
  });

  it('十个入口，网址里的关键词已编码，GitHub 两个指向这个仓库', () => {
    const links = communityLinks('MengTo/Skills');
    expect(links).toHaveLength(10);
    expect(new Set(links.map((link) => link.id)).size).toBe(10);
    expect(links.find((link) => link.id === 'xhs')?.url).toBe('https://www.xiaohongshu.com/search_result?keyword=MengTo%20Skills');
    expect(links.find((link) => link.id === 'issues')?.url).toContain('github.com/MengTo/Skills/issues');
    expect(links.find((link) => link.id === 'discussions')?.url).toBe('https://github.com/MengTo/Skills/discussions');
    for (const link of links) expect(link.url.startsWith('https://')).toBe(true);
  });

  it('给 Codex 的两段话：带上项目名和链接，要求不编造', () => {
    const community = buildCommunityPrompt({ fullName: 'harry0703/MoneyPrinterTurbo', htmlUrl: 'https://github.com/harry0703/MoneyPrinterTurbo' });
    expect(community).toContain('「MoneyPrinterTurbo」');
    expect(community).toContain('https://github.com/harry0703/MoneyPrinterTurbo');
    expect(community).toContain('不要编造');
    const install = buildInstallPrompt({ fullName: 'op7418/guizang-social-card-skill' });
    expect(install).toContain('https://github.com/op7418/guizang-social-card-skill');
    expect(install).toContain('先告诉我');
  });
});
