import { isValidElement, type ReactElement, type ReactNode } from 'react';
import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import type { LibraryDetail } from '@/lib/contracts';
import { Button } from '@/components/kit';
import { CommunityPanel, InstallPanel, PreviewPanel, ReadmePanel } from './detail-panels';

function textOf(html: string): string {
  return html
    .replace(/<span class="material-symbols-outlined[^>]*>[^<]*<\/span>/g, '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

const DETAIL: LibraryDetail = {
  id: 'r1',
  fullName: 'fox/tool',
  htmlUrl: 'https://github.com/fox/tool',
  description: 'A cover maker',
  language: 'TypeScript',
  starsCount: 120,
  starredAt: '2026-10-03T18:17:00Z',
  pushedAt: '2026-09-30T00:00:00Z',
  oneLiner: '做封面',
  form: 'skill',
  scenarioIds: [],
  readingStatus: 'unread',
  readingGroup: 'unseen',
  hasCard: true,
  confidence: 0.9,
  cardReview: 'suggested',
  whatItIs: '这是一个做封面的工具。',
  capabilities: [],
  problemsSolved: [],
  useWhen: [],
  notFor: [],
  howToTry: null,
  integratesWith: [],
  alternativesHint: [],
  inspirationForFox: null,
  risks: [],
  license: null,
  maturity: null,
  zhFriendly: null,
  keywords: [],
  topics: [],
  forksCount: 0,
  readmeExcerpt: null,
};

type Click = () => void;

/** 在元素树里找出所有 Button，返回它们的文字和点击函数（组件没有用 hook，可以直接当函数调用，不用 DOM）。 */
function buttonsIn(node: ReactNode): { label: string; onClick: Click | undefined }[] {
  const found: { label: string; onClick: Click | undefined }[] = [];
  const labelOf = (child: ReactNode): string => {
    if (typeof child === 'string' || typeof child === 'number') return String(child);
    if (Array.isArray(child)) return child.map(labelOf).join('');
    if (isValidElement(child)) return labelOf((child.props as { children?: ReactNode }).children);
    return '';
  };
  const walk = (child: ReactNode) => {
    if (Array.isArray(child)) {
      child.forEach(walk);
      return;
    }
    if (!isValidElement(child)) return;
    const element = child as ReactElement<{ children?: ReactNode; onClick?: Click }>;
    if (element.type === Button) {
      found.push({ label: labelOf(element.props.children).trim(), onClick: element.props.onClick });
    }
    walk(element.props.children);
  };
  walk(node);
  return found;
}

const NO_README = { state: 'ready', readme: null } as const;

describe('怎么试页：让 Codex 帮你装', () => {
  const props = {
    detail: DETAIL,
    readme: NO_README,
    steps: [],
    howFallback: '',
    hasDownload: false,
    scenarioName: '小红书',
    onCopyText: () => {},
    onCopyPrompt: () => {},
  };

  it('界面上讲清粘出去会发生什么：先查有没有、直接装进独立文件夹、要管理员密码 / 付费 / 登录会先问', () => {
    const text = textOf(renderToStaticMarkup(<InstallPanel {...props} />));
    expect(text).toContain('让 AI 助手帮你装');
    expect(text).toContain('复制一段话，粘给 AI 助手：它会先查你电脑上有没有、装在哪、要不要花钱；没有风险就直接装进独立文件夹，装完告诉你装了什么、怎么卸载。遇到要管理员密码、要付费或登录的情况，会先停下来问你。');
    expect(text).not.toContain('先读 README、告诉你要改动什么');
  });

  it('没有下载项目说明时，说清为什么找不到命令、怎么补上（顶栏的「同步」，或「设置 → 账号与同步」里的「下载项目说明」）；用「项目说明」不用「README 缓存」', () => {
    const text = textOf(renderToStaticMarkup(<InstallPanel {...props} />));
    expect(text).toContain('还没下载这个工具的项目说明，所以找不到安装命令。点顶栏的「同步」，或到「设置 → 账号与同步」点「下载项目说明」，就会补上。');
    expect(text).not.toContain('缓存');
    // 原来指错了地方：自动整理与备份里没有下载项目说明的入口
    expect(text).not.toContain('自动整理与备份');
    expect(text).not.toContain('会自动下载');
  });

  it('下载安装包那一块用人话：发行版（Releases）、.dmg（Mac 安装包）', () => {
    const text = textOf(renderToStaticMarkup(<InstallPanel {...props} hasDownload />));
    expect(text).toContain('下载安装包');
    expect(text).toContain('在 GitHub 页面右侧找「Releases（发行版）」，下载 .dmg（Mac 安装包）');
  });

  it('「下载安装包」那张卡只在项目说明里真的提到了安装包 / 发行版时才显示：没找到命令也没找到安装包，就不显示', () => {
    const none = textOf(renderToStaticMarkup(<InstallPanel {...props} />));
    expect(none).not.toContain('下载安装包');
    expect(none).not.toContain('Releases');
    // 下载了项目说明、里面既没有命令也没有安装包的迹象：同样不显示（原来这时一律显示一张）
    const downloaded = textOf(renderToStaticMarkup(
      <InstallPanel {...props} readme={{ state: 'ready', readme: { rawMarkdown: '# tool', sourcePath: 'README.md' } }} />,
    ));
    expect(downloaded).not.toContain('下载安装包');
    expect(downloaded).toContain('让 AI 助手帮你装');
  });

  it('找到了安装包 / 发行版的迹象就显示；有命令、没有安装包的迹象时不显示', () => {
    const found = renderToStaticMarkup(<InstallPanel {...props} hasDownload />);
    expect(textOf(found)).toContain('下载安装包');
    expect(found).toContain('href="https://github.com/fox/tool/releases/latest"');
    const withStep = renderToStaticMarkup(
      <InstallPanel {...props} steps={[{ title: 'npm', command: 'npm i tool', kind: 'npm' }]} />,
    );
    expect(textOf(withStep)).toContain('npm i tool');
    expect(textOf(withStep)).not.toContain('下载安装包');
    // 有命令、也有安装包的迹象：两个都在
    const both = renderToStaticMarkup(
      <InstallPanel {...props} hasDownload steps={[{ title: 'npm', command: 'npm i tool', kind: 'npm' }]} />,
    );
    expect(textOf(both)).toContain('npm i tool');
    expect(textOf(both)).toContain('下载安装包');
  });

  it('还在读取项目说明时只画占位，不先说「没找到」', () => {
    const text = textOf(renderToStaticMarkup(<InstallPanel {...props} readme={{ state: 'loading' }} />));
    expect(text).not.toContain('下载安装包');
    expect(text).not.toContain('找不到安装命令');
  });

  it('点「复制」交出来的是装好并试用的那段话，并标明这是安装类（页面据此给安装类的提示）', () => {
    const copied: { text: string; kind: string }[] = [];
    const tree = InstallPanel({ ...props, onCopyPrompt: (text, kind) => copied.push({ text, kind }) });
    const copy = buttonsIn(tree).filter((button) => button.label === '复制');
    expect(copy).toHaveLength(1);
    copy[0].onClick?.();
    expect(copied).toHaveLength(1);
    expect(copied[0].kind).toBe('install');
    expect(copied[0].text).toContain('请在这台 Mac 上安装并跑通「tool」');
    expect(copied[0].text).toContain('类型：技能');
  });
});

describe('社区页：让 AI 助手去查别人怎么用', () => {
  it('点「复制给 AI 助手」交出来的是查资料的话，标明是 community：它不装东西，不能套用安装类的提示', () => {
    const copied: { text: string; kind: string }[] = [];
    const tree = CommunityPanel({ detail: DETAIL, scenarioName: '小红书', onCopyPrompt: (text, kind) => copied.push({ text, kind }) });
    const copy = buttonsIn(tree).filter((button) => button.label === '复制给 AI 助手');
    expect(copy).toHaveLength(1);
    copy[0].onClick?.();
    expect(copied).toHaveLength(1);
    expect(copied[0].kind).toBe('community');
    expect(copied[0].text).toContain('请用 web-search 调查开源项目「tool」');
    expect(copied[0].text).not.toContain('请在这台 Mac 上安装并跑通');
  });
});

describe('项目说明页', () => {
  it('还没下载时，说清楚、并给出真实存在的两个入口：顶栏的「同步」，或「设置 → 账号与同步」里的「下载项目说明」', () => {
    const text = textOf(renderToStaticMarkup(<ReadmePanel detail={DETAIL} readme={NO_README} />));
    expect(text).toContain('还没下载这个工具的项目说明。 点顶栏的「同步」，或到「设置 → 账号与同步」点「下载项目说明」，就会补上。');
    expect(text).not.toContain('缓存');
    // 原来写的是「设置 → 自动整理与备份」里会自动下载：指错了地方，也是半截句
    expect(text).not.toContain('自动整理与备份');
    expect(text).not.toContain('会自动下载');
    expect(text).not.toContain('所以找不到安装命令');
  });

  it('读取中画占位；读出错说没读到', () => {
    const loading = textOf(renderToStaticMarkup(<ReadmePanel detail={DETAIL} readme={{ state: 'loading' }} />));
    expect(loading).not.toContain('还没下载');
    const failed = textOf(renderToStaticMarkup(<ReadmePanel detail={DETAIL} readme={{ state: 'error', message: '读不出来' }} />));
    expect(failed).toContain('没有读到项目说明：读不出来');
    expect(failed).not.toContain('还没下载');
  });
});

describe('预览页', () => {
  const NO_IMAGES: never[] = [];
  const DOWNLOADED = { state: 'ready', readme: { rawMarkdown: '# tool\n只有文字', sourcePath: 'README.md' } } as const;

  it('还没下载项目说明：说「还没下载项目说明，暂时看不到截图。」，并说怎么补上；不说它「没有截图」', () => {
    const html = renderToStaticMarkup(<PreviewPanel detail={DETAIL} readme={NO_README} images={NO_IMAGES} />);
    const text = textOf(html);
    expect(text).toContain('还没下载项目说明，暂时看不到截图。');
    expect(text).toContain('点顶栏的「同步」，或到「设置 → 账号与同步」点「下载项目说明」，就会补上。');
    expect(text).not.toContain('项目说明里没有截图');
    // 没下载时，「这个工具没有放界面或效果图」「切到项目说明读原文」都不成立
    expect(text).not.toContain('没有放界面或效果图');
    expect(text).not.toContain('切到「项目说明」读它的原文');
    // 去 GitHub 看看的入口还在
    expect(html).toContain('href="https://github.com/fox/tool"');
  });

  it('已经下载、里面确实没有图：保留原来那句', () => {
    const text = textOf(renderToStaticMarkup(<PreviewPanel detail={DETAIL} readme={DOWNLOADED} images={NO_IMAGES} />));
    expect(text).toContain('项目说明里没有截图');
    expect(text).toContain('这个工具没有放界面或效果图。可以去 GitHub 看看，或者切到「项目说明」读它的原文。');
    expect(text).not.toContain('还没下载');
  });

  it('读取中画占位，读出错说没读到；都不说「没有截图」', () => {
    const loading = textOf(renderToStaticMarkup(<PreviewPanel detail={DETAIL} readme={{ state: 'loading' }} images={NO_IMAGES} />));
    expect(loading).not.toContain('截图');
    const failed = textOf(renderToStaticMarkup(<PreviewPanel detail={DETAIL} readme={{ state: 'error', message: '读不出来' }} images={NO_IMAGES} />));
    expect(failed).toContain('没有读到项目说明：读不出来');
    expect(failed).not.toContain('没有截图');
  });

  it('有图就列出来，不说没有', () => {
    const html = renderToStaticMarkup(
      <PreviewPanel detail={DETAIL} readme={DOWNLOADED} images={[{ url: 'https://example.com/a.png', alt: '界面' }]} />,
    );
    expect(textOf(html)).toContain('来自项目说明的截图，共 1 张');
    expect(textOf(html)).not.toContain('没有截图');
  });
});
