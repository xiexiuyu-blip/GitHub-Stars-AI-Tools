/**
 * 仓库的「看得见」的那一部分：头像、封面、语言颜色、场景色，以及从 README 里抽出来的截图和安装命令、
 * 去各平台看讨论的入口。全是纯函数（不碰 React、不联网），方便在 node 里测。
 */

/* ─────────────── 头像与颜色 ─────────────── */

export function ownerOf(fullName: string): string {
  const slash = fullName.indexOf('/');
  return (slash >= 0 ? fullName.slice(0, slash) : '').trim();
}

export function repoNameOf(fullName: string): string {
  const slash = fullName.indexOf('/');
  return (slash >= 0 ? fullName.slice(slash + 1) : fullName).trim();
}

/** GitHub 的头像地址（所有者的头像）。名字不合法返回 null，界面退回首字母方块。 */
export function avatarUrl(fullName: string, px = 80): string | null {
  const owner = ownerOf(fullName);
  if (!owner || !/^[A-Za-z0-9._-]+$/.test(owner)) return null;
  return `https://github.com/${owner}.png?size=${Math.max(16, Math.min(460, Math.round(px)))}`;
}

/** 同一段文字永远得到同一个色相（0–359）。FNV-1a。 */
export function hueOf(text: string): number {
  let hash = 0x811c9dc5;
  for (const char of text.toLowerCase()) {
    hash ^= char.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash % 360;
}

/** 没有封面图时的底：由名字决定的浅色渐变，列表里每个仓库颜色不同但稳定。 */
export function coverGradient(text: string): string {
  const hue = hueOf(text);
  return `linear-gradient(135deg, hsl(${hue} 70% 91%), hsl(${(hue + 42) % 360} 68% 83%))`;
}

const LANGUAGE_COLORS: Record<string, string> = {
  typescript: '#3178c6',
  javascript: '#f1e05a',
  python: '#3572a5',
  rust: '#dea584',
  go: '#00add8',
  java: '#b07219',
  c: '#555555',
  'c++': '#f34b7d',
  'c#': '#178600',
  ruby: '#701516',
  php: '#4f5d95',
  swift: '#f05138',
  kotlin: '#a97bff',
  shell: '#89e051',
  html: '#e34c26',
  css: '#563d7c',
  scss: '#c6538c',
  vue: '#41b883',
  svelte: '#ff3e00',
  astro: '#ff5a03',
  dart: '#00b4ab',
  lua: '#000080',
  zig: '#ec915c',
  scala: '#c22d40',
  elixir: '#6e4a7e',
  haskell: '#5e5086',
  r: '#198ce7',
  nix: '#7e7eff',
  tex: '#3d6117',
  mdx: '#fcb32c',
  markdown: '#083fa1',
  dockerfile: '#384d54',
  powershell: '#012456',
  'jupyter notebook': '#da5b0b',
  'objective-c': '#438eff',
  mojo: '#ff4c1f',
};

export function languageColor(language: string | null | undefined): string | null {
  const key = language?.trim().toLowerCase();
  if (!key) return null;
  return LANGUAGE_COLORS[key] ?? null;
}

/** 十三个业务场景各一个色相。键可以是 slug（xhs）或场景 id（scn-xhs）。 */
const SCENARIO_HUES: Record<string, number> = {
  xhs: 350,
  wx: 142,
  dy: 268,
  img: 320,
  vid: 28,
  ppt: 199,
  wr: 232,
  res: 188,
  data: 168,
  kb: 44,
  auto: 220,
  ec: 8,
  seo: 100,
};

export function scenarioHue(key: string | null | undefined): number {
  const slug = (key ?? '').trim().replace(/^scn-/, '');
  if (slug in SCENARIO_HUES) return SCENARIO_HUES[slug];
  return hueOf(slug || 'scenario');
}

export function releasesUrl(fullName: string): string {
  return `https://github.com/${fullName}/releases/latest`;
}

/* ─────────────── 从 README 里抽截图 ─────────────── */

export type PreviewImage = { url: string; alt: string };

const MD_IMAGE = /!\[([^\]]*)\]\(\s*<?([^)\s>]+)/g;
const HTML_IMAGE = /<img\b[^>]*?>/gi;
const HTML_SRC = /\bsrc\s*=\s*["']([^"']+)["']/i;
const HTML_ALT = /\balt\s*=\s*["']([^"']*)["']/i;

/** 徽章、统计图、分隔线之类，不是「这个工具长什么样」。 */
const NOT_A_PREVIEW =
  /(shields\.io|badge|badgen|travis|codecov|circleci|\/actions\/|\/workflows\/|star-history|trendshift|hits\.seeyoufarm|visitor-badge|komarev|contrib\.rocks|opencollective|capsule-render|readme-stats|api\.star|rainbow|divider|separator|spacer|\/line[._-]|[._-]line\.|emoji|sponsor)/i;
/** 只在没有别的图时才用：logo、图标。 */
const LOGO_LIKE = /(logo|icon|favicon|avatar|banner-small)/i;

export function resolveAssetUrl(raw: string, fullName: string): string | null {
  const value = raw.trim();
  if (!value || value.startsWith('data:') || value.startsWith('#')) return null;
  if (value.startsWith('//')) return `https:${value}`;
  if (/^https?:\/\//i.test(value)) {
    // github.com/o/r/blob/ref/path → raw.githubusercontent.com，浏览器才能直接当图片显示
    const blob = value.match(/^https:\/\/github\.com\/([^/]+)\/([^/]+)\/blob\/([^/]+)\/(.+?)(\?.*)?$/i);
    if (blob) return `https://raw.githubusercontent.com/${blob[1]}/${blob[2]}/${blob[3]}/${blob[4]}`;
    return value;
  }
  if (/^[a-z][a-z0-9+.-]*:/i.test(value)) return null;
  const path = value
    .replace(/^\.\//, '')
    .replace(/^\/+/, '')
    .split('/')
    .filter((part) => part && part !== '.' && part !== '..')
    .join('/')
    .replace(/ /g, '%20');
  if (!path) return null;
  return `https://raw.githubusercontent.com/${fullName}/HEAD/${path}`;
}

type RawImage = { url: string; alt: string };

function rawImages(markdown: string): RawImage[] {
  const found: Array<RawImage & { at: number }> = [];
  for (const match of markdown.matchAll(MD_IMAGE)) {
    found.push({ url: match[2], alt: match[1] ?? '', at: match.index ?? 0 });
  }
  for (const match of markdown.matchAll(HTML_IMAGE)) {
    const tag = match[0];
    const src = tag.match(HTML_SRC)?.[1];
    if (!src) continue;
    found.push({ url: src, alt: tag.match(HTML_ALT)?.[1] ?? '', at: match.index ?? 0 });
  }
  return found.sort((a, b) => a.at - b.at).map(({ url, alt }) => ({ url, alt }));
}

/** 按 README 里出现的顺序取前几张「像截图的图」。去掉徽章和 svg；一张都没有时才放宽到 logo。 */
export function extractPreviewImages(markdown: string, fullName: string, limit = 8): PreviewImage[] {
  const strict: PreviewImage[] = [];
  const loose: PreviewImage[] = [];
  const seen = new Set<string>();
  for (const image of rawImages(markdown ?? '')) {
    const resolved = resolveAssetUrl(image.url, fullName);
    if (!resolved || seen.has(resolved)) continue;
    if (NOT_A_PREVIEW.test(resolved) || /\.svg(\?|$)/i.test(resolved)) continue;
    seen.add(resolved);
    const entry = { url: resolved, alt: image.alt.trim() };
    (LOGO_LIKE.test(resolved) ? loose : strict).push(entry);
  }
  return (strict.length > 0 ? strict : loose).slice(0, Math.max(0, limit));
}

/* ─────────────── 从 README 里抽安装命令 ─────────────── */

export type InstallKind = 'docker' | 'npm' | 'python' | 'brew' | 'git' | 'curl' | 'go' | 'cargo' | 'other';
export type InstallStep = { title: string; command: string; kind: InstallKind };

const INSTALL_HEADING =
  /(install|setup|set up|quick\s*start|getting\s*started|get started|usage|run\b|deploy|download|安装|部署|快速开始|快速上手|使用|运行|启动|下载)/i;
const COMMAND_START =
  /^(\$\s*|>\s*|PS [^>]*>\s*)?(sudo\s+)?(npm|npx|pnpm|yarn|bun|bunx|pip3?|pipx|uv|uvx|python3?|brew|docker|docker-compose|git|curl|wget|cargo|go|conda|mamba|apt|apt-get|dnf|yum|winget|choco|scoop|make|bash|sh|source|cd|claude|codex|\.\/|\.\\|iwr|irm|powershell)\b/i;
const INSTALLISH =
  /^(\$\s*)?(sudo\s+)?(npm\s+(i|install|run)|npx\s|pnpm\s+(i|add|install|dlx)|yarn\s+(add|global)|bunx?\s|pip3?\s+install|pipx\s+install|uv\s+(pip|tool|run|sync)|uvx\s|brew\s+install|docker\s+(run|compose|pull)|docker-compose|git\s+clone|curl\s|wget\s|cargo\s+(install|run)|go\s+install|conda\s+install|claude\s+|codex\s+)/i;
const SHELL_LANGS = new Set(['', 'bash', 'sh', 'shell', 'zsh', 'console', 'terminal', 'powershell', 'ps1', 'cmd', 'bat', 'batch', 'text', 'txt', 'fish']);

function kindOf(command: string): InstallKind {
  // 跳过空行、注释和 cd：真正干活的是后面那条
  const line = command
    .split('\n')
    .map((item) => item.trim())
    .find((item) => item && !item.startsWith('#') && !/^cd\s/i.test(item)) ?? '';
  const first = line.replace(/^(\$\s*|>\s*)/, '').replace(/^sudo\s+/, '').split(/\s+/)[0]?.toLowerCase() ?? '';
  if (/^docker/.test(first)) return 'docker';
  if (['npm', 'npx', 'pnpm', 'yarn', 'bun', 'bunx'].includes(first)) return 'npm';
  if (['pip', 'pip3', 'pipx', 'uv', 'uvx', 'python', 'python3', 'conda', 'mamba'].includes(first)) return 'python';
  if (first === 'brew') return 'brew';
  if (first === 'git') return 'git';
  if (['curl', 'wget', 'iwr', 'irm'].includes(first)) return 'curl';
  if (first === 'go') return 'go';
  if (first === 'cargo') return 'cargo';
  return 'other';
}

function cleanHeading(raw: string): string {
  return raw
    .replace(/<[^>]+>/g, '')
    .replace(/[*_`#]/g, '')
    .replace(/^\s*[\d①②③④⑤⑥⑦⑧⑨一二三四五六七八九十]+[.、)）\s]+/, '')
    .replace(/\s+/g, ' ')
    .trim();
}

function cleanCommand(body: string): string {
  const lines = body
    .replace(/\r/g, '')
    .split('\n')
    .map((line) => line.replace(/\s+$/, ''))
    .map((line) => line.replace(/^(\$|>)\s+/, ''));
  while (lines.length > 0 && lines[0].trim() === '') lines.shift();
  while (lines.length > 0 && lines[lines.length - 1].trim() === '') lines.pop();
  const kept = lines.slice(0, 10);
  const text = kept.join('\n');
  const clipped = text.length > 500 ? `${text.slice(0, 500)}…` : text;
  return lines.length > 10 ? `${clipped}\n…` : clipped;
}

function looksLikeCommand(lang: string, body: string): boolean {
  const first = body.split('\n').find((line) => line.trim() !== '')?.trim() ?? '';
  if (!first) return false;
  if (SHELL_LANGS.has(lang)) return COMMAND_START.test(first) || /^[A-Za-z0-9_.-]+\s+\S/.test(first);
  return INSTALLISH.test(first);
}

type Block = { index: number; lang: string; body: string };

function fencedBlocks(text: string, offset: number): Block[] {
  const blocks: Block[] = [];
  const fence = /^[ \t]*(?:```|~~~)[ \t]*([\w+#.-]*)[^\n]*\n([\s\S]*?)^[ \t]*(?:```|~~~)[ \t]*$/gm;
  for (const match of text.matchAll(fence)) {
    blocks.push({ index: offset + (match.index ?? 0), lang: (match[1] ?? '').toLowerCase(), body: match[2] ?? '' });
  }
  return blocks;
}

/** 最多取几条安装 / 上手命令：优先「安装 / 快速开始 / 使用」标题下面的代码块，找不到再退回到全文里最像安装命令的。 */
export function extractInstallSteps(markdown: string, limit = 3): InstallStep[] {
  const text = (markdown ?? '').replace(/\r\n/g, '\n');
  const headings = [...text.matchAll(/^(#{1,4})[ \t]+(.+?)[ \t]*#*[ \t]*$/gm)];
  const steps: InstallStep[] = [];
  const seen = new Set<string>();
  const push = (title: string, body: string) => {
    const command = cleanCommand(body);
    if (!command || seen.has(command)) return;
    seen.add(command);
    steps.push({ title: title || '安装', command, kind: kindOf(command) });
  };

  for (let i = 0; i < headings.length && steps.length < limit; i += 1) {
    const title = cleanHeading(headings[i][2]);
    if (!INSTALL_HEADING.test(title)) continue;
    const start = (headings[i].index ?? 0) + headings[i][0].length;
    const end = headings[i + 1]?.index ?? text.length;
    for (const block of fencedBlocks(text.slice(start, end), start)) {
      if (looksLikeCommand(block.lang, block.body)) push(title, block.body);
      if (steps.length >= limit) break;
    }
  }

  if (steps.length === 0) {
    for (const block of fencedBlocks(text, 0)) {
      const first = block.body.split('\n').find((line) => line.trim() !== '')?.trim() ?? '';
      if (INSTALLISH.test(first)) push('安装', block.body);
      if (steps.length >= limit) break;
    }
  }
  return steps.slice(0, Math.max(0, limit));
}

/** README 里有没有「下载安装包」的迹象（桌面应用常见）。 */
export function hasDownloadHint(markdown: string): boolean {
  return /(\.(exe|dmg|pkg|msi|appimage|deb|apk)\b|releases?\b|下载|安装包)/i.test(markdown ?? '');
}

/* ─────────────── 去别处看别人怎么用 ─────────────── */

export type CommunityLink = {
  id: string;
  label: string;
  url: string;
  hint: string;
  icon: string;
};

const GENERIC_NAMES = new Set(['skills', 'skill', 'plugin', 'plugins', 'tools', 'tool', 'agent', 'agents', 'app', 'apps', 'cli', 'core', 'docs', 'demo', 'awesome']);

/** 搜索词：仓库名太短或太通用（Skills、agent-plugin）就带上所有者，免得搜出一堆无关的。 */
export function searchTerm(fullName: string): string {
  const repo = repoNameOf(fullName);
  const owner = ownerOf(fullName);
  const generic = repo.length < 6 || GENERIC_NAMES.has(repo.toLowerCase()) || repo.toLowerCase().split(/[-_]/).every((part) => GENERIC_NAMES.has(part));
  return generic && owner ? `${owner} ${repo}` : repo;
}

export function communityLinks(fullName: string): CommunityLink[] {
  const q = encodeURIComponent(searchTerm(fullName));
  const repoUrl = encodeURIComponent(`https://github.com/${fullName}`);
  return [
    { id: 'xhs', label: '小红书', url: `https://www.xiaohongshu.com/search_result?keyword=${q}`, hint: '看博主怎么用、效果怎么样', icon: 'photo_library' },
    { id: 'bilibili', label: 'B 站', url: `https://search.bilibili.com/all?keyword=${q}`, hint: '看演示和教程视频', icon: 'smart_display' },
    { id: 'zhihu', label: '知乎', url: `https://www.zhihu.com/search?type=content&q=${q}`, hint: '看评测和对比', icon: 'forum' },
    { id: 'juejin', label: '掘金', url: `https://juejin.cn/search?query=${q}`, hint: '看中文技术文章', icon: 'article' },
    { id: 'x', label: 'X', url: `https://x.com/search?q=${q}&f=live`, hint: '看最新的讨论', icon: 'tag' },
    { id: 'reddit', label: 'Reddit', url: `https://www.reddit.com/search/?q=${q}`, hint: '看使用心得和吐槽', icon: 'groups' },
    { id: 'youtube', label: 'YouTube', url: `https://www.youtube.com/results?search_query=${q}`, hint: '看演示视频', icon: 'play_circle' },
    { id: 'hn', label: 'Hacker News', url: `https://hn.algolia.com/?q=${repoUrl}&type=story`, hint: '看有没有被讨论过', icon: 'newspaper' },
    { id: 'issues', label: 'GitHub Issues', url: `https://github.com/${fullName}/issues?q=is%3Aissue+sort%3Acomments-desc`, hint: '看大家遇到什么问题', icon: 'bug_report' },
    { id: 'discussions', label: 'GitHub Discussions', url: `https://github.com/${fullName}/discussions`, hint: '看使用经验和提问', icon: 'chat' },
  ];
}

export function buildCommunityPrompt(input: { fullName: string; htmlUrl?: string | null }): string {
  const url = input.htmlUrl?.trim() || `https://github.com/${input.fullName}`;
  return [
    `请用 web-search 帮我查一下开源项目「${repoNameOf(input.fullName)}」（${url}）在网上的真实使用讨论，重点看：`,
    '1. 谁在用、拿它做什么：给出 3–5 个具体的场景或案例，每条附原帖链接；',
    '2. 实际用起来的体验：好用的地方、踩过的坑、常见的报错和解决办法；',
    '3. 有没有更好、或更常被推荐的替代品；',
    '4. 中文社区（小红书、知乎、B 站、微信公众号、V2EX、掘金）和英文社区（X、Reddit、Hacker News、YouTube、GitHub Issues / Discussions）分开列。',
    '只引用你真正查到的内容，每条都带链接；查不到就直说，不要编造。',
    '最后用不超过 300 字给我一个结论：它现在值不值得我花时间试。',
  ].join('\n');
}

export function buildInstallPrompt(input: { fullName: string; htmlUrl?: string | null }): string {
  const url = input.htmlUrl?.trim() || `https://github.com/${input.fullName}`;
  return [
    `请帮我在这台 Mac 上安装并跑通「${repoNameOf(input.fullName)}」（${url}）。`,
    '1. 先读它 README 里的安装 / 快速开始部分，确认系统要求和依赖；',
    '2. 动手之前先告诉我：会装到哪里、会改动什么、需要联网下载什么；有多种安装方式时选最简单、最容易撤销的；',
    '3. 不确定的步骤先问我，不要编造命令；需要管理员权限或付费账号时停下来等我；',
    '4. 装好后用一个最小的例子验证它真的能跑，再告诉我怎么用它做我手头的事。',
  ].join('\n');
}
