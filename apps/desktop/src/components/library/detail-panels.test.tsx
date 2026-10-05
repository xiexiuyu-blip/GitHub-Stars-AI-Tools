import { isValidElement, type ReactElement, type ReactNode } from 'react';
import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import type { LibraryDetail } from '@/lib/contracts';
import { Button } from '@/components/kit';
import { CommunityPanel, InstallPanel, ReadmePanel } from './detail-panels';

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
    expect(text).toContain('让 Codex 帮你装');
    expect(text).toContain('复制一段话，粘给 Codex：它会先查你电脑上有没有、装在哪、要不要花钱；没有风险就直接装进独立文件夹，装完告诉你装了什么、怎么卸载。遇到要管理员密码、要付费或登录的情况，会先停下来问你。');
    expect(text).not.toContain('先读 README、告诉你要改动什么');
  });

  it('没有下载项目说明时，说清为什么找不到命令、之后会自动下载；用「项目说明」不用「README 缓存」', () => {
    const text = textOf(renderToStaticMarkup(<InstallPanel {...props} />));
    expect(text).toContain('还没下载这个工具的项目说明，所以找不到安装命令。同步后或在「设置 → 自动整理与备份」里会自动下载');
    expect(text).not.toContain('缓存');
  });

  it('下载安装包那一块用人话：发行版（Releases）、.dmg（Mac 安装包）', () => {
    const text = textOf(renderToStaticMarkup(<InstallPanel {...props} />));
    expect(text).toContain('在 GitHub 页面右侧找「Releases（发行版）」，下载 .dmg（Mac 安装包）');
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

describe('社区页：让 Codex 去查别人怎么用', () => {
  it('点「复制给 Codex」交出来的是查资料的话，标明是 community：它不装东西，不能套用安装类的提示', () => {
    const copied: { text: string; kind: string }[] = [];
    const tree = CommunityPanel({ detail: DETAIL, scenarioName: '小红书', onCopyPrompt: (text, kind) => copied.push({ text, kind }) });
    const copy = buttonsIn(tree).filter((button) => button.label === '复制给 Codex');
    expect(copy).toHaveLength(1);
    copy[0].onClick?.();
    expect(copied).toHaveLength(1);
    expect(copied[0].kind).toBe('community');
    expect(copied[0].text).toContain('请用 web-search 调查开源项目「tool」');
    expect(copied[0].text).not.toContain('请在这台 Mac 上安装并跑通');
  });
});

describe('项目说明页', () => {
  it('还没下载时，标题和说明用「项目说明」，说明下载时机', () => {
    const text = textOf(renderToStaticMarkup(<ReadmePanel detail={DETAIL} readme={NO_README} />));
    expect(text).toContain('还没下载这个工具的项目说明');
    expect(text).toContain('同步后或在「设置 → 自动整理与备份」里会自动下载');
    expect(text).not.toContain('缓存');
  });
});
