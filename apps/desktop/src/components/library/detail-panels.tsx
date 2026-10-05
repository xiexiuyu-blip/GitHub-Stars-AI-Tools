import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import type { LibraryDetail } from '@/lib/contracts';
import { INSTALL_COPY_HINT, buildCommunityPrompt, buildInstallPrompt } from '@/lib/codex-prompts';
import { communityNote, groupCommunity, itemMeta, stateLabel } from '@/lib/community-view';
import {
  INSTALL_NEEDS_README,
  README_DOWNLOAD_HOW,
  README_NOT_DOWNLOADED_TITLE,
  noScreenshotsNote,
  relativeDays,
  type CopyKind,
} from '@/lib/item-format';
import { useRepoCommunity } from '@/hooks/use-repo-community';
import {
  communityLinks,
  releasesUrl,
  type InstallStep,
  type PreviewImage,
} from '@/lib/repo-assets';
import { ReadmeRenderer } from '@/components/readme-renderer';
import { Button, CoverImage, EmptyState, Spinner, Tag } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';

type ReadmeState =
  | { state: 'loading' }
  | { state: 'ready'; readme: { rawMarkdown: string; sourcePath: string } | null }
  | { state: 'error'; message: string };

/** 小标题：带颜色的图标底 + 文字。几个区块靠它在视觉上分开。 */
export function SectionTitle(props: { icon: string; hue: number; children: string }) {
  return (
    <h3 className="m-0 mb-2 flex items-center gap-2 text-[13px] font-semibold text-foreground">
      <span className="hue-tile grid size-6 place-items-center rounded-md" style={{ '--h': props.hue } as React.CSSProperties}>
        <Icon name={props.icon} size={15} fill />
      </span>
      {props.children}
    </h3>
  );
}

export function SoftCard(props: { children: React.ReactNode; className?: string }) {
  return <div className={cn('rounded-lg border border-border bg-card p-4', props.className)}>{props.children}</div>;
}

function PanelSkeleton() {
  return (
    <div className="flex flex-col gap-3" aria-hidden="true">
      <div className="h-24 animate-pulse rounded-xl bg-muted" />
      <div className="h-24 animate-pulse rounded-xl bg-muted" />
    </div>
  );
}

function panelError(message: string) {
  return <p className="text-sm text-on-surface-variant">没有读到项目说明：{message}</p>;
}

const KIND_ICON: Record<InstallStep['kind'], string> = {
  docker: 'deployed_code',
  npm: 'terminal',
  python: 'terminal',
  brew: 'terminal',
  git: 'terminal',
  curl: 'download',
  go: 'terminal',
  cargo: 'terminal',
  other: 'terminal',
};

/* ─────────────── 安装 ─────────────── */

export function InstallPanel(props: {
  detail: LibraryDetail;
  readme: ReadmeState;
  steps: InstallStep[];
  howFallback: string;
  hasDownload: boolean;
  scenarioName?: string | null;
  onCopyText: (text: string) => void;
  onCopyPrompt: (text: string, kind: CopyKind) => void;
}) {
  const { detail } = props;
  if (props.readme.state === 'loading') return <PanelSkeleton />;
  // 「下载安装包」那张卡只在项目说明里真的提到了安装包 / 发行版时才显示；没找到就不显示（不再没找到命令就一律放一张）
  const showReleases = props.hasDownload;
  return (
    <div className="flex flex-col gap-3">
      {props.steps.map((step) => (
        <div key={step.command} className="overflow-hidden rounded-lg border border-border bg-card">
          <div className="flex items-center gap-2 border-b border-border bg-surface-container-low px-3 py-2">
            <Icon name={KIND_ICON[step.kind]} size={16} className="text-on-surface-variant" />
            <span className="min-w-0 truncate text-[13px] font-medium text-foreground">{step.title}</span>
            <span className="flex-1" />
            <Button size="sm" onClick={() => props.onCopyText(step.command)} aria-label={`复制命令：${step.title}`}>
              <Icon name="content_copy" size={14} />
              复制
            </Button>
          </div>
          <pre className="code-block m-0 px-3.5 py-3 text-xs leading-[1.75] break-words whitespace-pre-wrap">{step.command}</pre>
        </div>
      ))}

      {props.steps.length === 0 && props.howFallback ? (
        <div className="overflow-hidden rounded-lg border border-border bg-card">
          <div className="flex items-center gap-2 border-b border-border bg-surface-container-low px-3 py-2">
            <Icon name="auto_awesome" size={16} className="text-accent-foreground" />
            <span className="text-[13px] font-medium text-foreground">怎么试</span>
            <span className="flex-1" />
            <Button size="sm" onClick={() => props.onCopyText(props.howFallback)}>
              <Icon name="content_copy" size={14} />
              复制
            </Button>
          </div>
          <pre className="code-block m-0 px-3.5 py-3 text-xs leading-[1.75] break-words whitespace-pre-wrap">{props.howFallback}</pre>
        </div>
      ) : null}

      {showReleases ? (
        <a
          href={releasesUrl(detail.fullName)}
          target="_blank"
          rel="noopener noreferrer"
          className="flex items-center gap-3 rounded-lg border border-border bg-card p-3.5 transition-colors hover:border-input hover:bg-surface-container-low"
        >
          <span className="hue-tile grid size-10 shrink-0 place-items-center rounded-[10px]" style={{ '--h': 148 } as React.CSSProperties}>
            <Icon name="download" size={22} fill />
          </span>
          <span className="min-w-0 flex-1">
            <b className="block text-sm font-semibold text-foreground">下载安装包</b>
            <span className="text-xs leading-[1.6] text-on-surface-variant">在 GitHub 页面右侧找「Releases（发行版）」，下载 .dmg（Mac 安装包）</span>
          </span>
          <Icon name="open_in_new" size={16} className="shrink-0 text-on-surface-variant" />
        </a>
      ) : null}

      {props.steps.length === 0 && !props.howFallback && props.readme.state === 'ready' && !props.readme.readme ? (
        <p className="text-sm text-on-surface-variant">{INSTALL_NEEDS_README}</p>
      ) : null}

      <SoftCard className="flex items-center gap-3">
        <span className="hue-tile grid size-10 shrink-0 place-items-center rounded-[10px]" style={{ '--h': 222 } as React.CSSProperties}>
          <Icon name="smart_toy" size={22} fill />
        </span>
        <span className="min-w-0 flex-1">
          <b className="block text-sm font-semibold text-foreground">让 Codex 帮你装</b>
          <span className="text-xs leading-[1.6] text-on-surface-variant">{INSTALL_COPY_HINT}</span>
        </span>
        <Button onClick={() => props.onCopyPrompt(buildInstallPrompt({ fullName: detail.fullName, htmlUrl: detail.htmlUrl, form: detail.form, scenarioName: props.scenarioName }), 'install')}>
          <Icon name="content_copy" size={16} />
          复制
        </Button>
      </SoftCard>
    </div>
  );
}

/* ─────────────── 预览（项目说明里的截图） ─────────────── */

export function PreviewPanel(props: { detail: LibraryDetail; readme: ReadmeState; images: PreviewImage[] }) {
  const [open, setOpen] = useState<number | null>(null);
  // 加载失败的（比如其实是视频、已被删除的图）直接从相册里拿掉，不留空白格子
  const [failed, setFailed] = useState<ReadonlySet<string>>(new Set());
  const images = props.images.filter((image) => !failed.has(image.url));
  if (props.readme.state === 'loading') return <PanelSkeleton />;
  if (props.readme.state === 'error') return panelError(props.readme.message);
  if (images.length === 0) {
    // 还没下载项目说明：不是「它没放图」，是还没看，不能说没有
    const empty = noScreenshotsNote(props.readme.readme !== null);
    return (
      <EmptyState
        icon="hide_image"
        title={empty.title}
        description={empty.description}
        action={props.detail.htmlUrl ? (
          <a className="text-sm text-accent-foreground hover:underline" href={props.detail.htmlUrl} target="_blank" rel="noopener noreferrer">打开 GitHub</a>
        ) : undefined}
      />
    );
  }
  return (
    <>
      <p className="mb-2.5 text-xs text-on-surface-variant">来自项目说明的截图，共 {images.length} 张，点一张放大。</p>
      <div className="grid grid-cols-2 gap-2.5">
        {images.map((image, index) => (
          <button
            key={image.url}
            type="button"
            className="group relative block overflow-hidden rounded-lg border border-border bg-card text-left transition-shadow hover:shadow-[0_4px_12px_rgba(31,35,41,0.12)]"
            onClick={() => setOpen(index)}
            aria-label={`放大第 ${index + 1} 张截图`}
          >
            <CoverImage
              src={image.url}
              seed={`${props.detail.fullName}#${index}`}
              alt={image.alt}
              className="aspect-[16/10] w-full"
              letter={false}
              onFail={() => setFailed((current) => new Set(current).add(image.url))}
            />
            <span className="absolute right-2 bottom-2 hidden size-7 place-items-center rounded-md bg-black/55 text-white group-hover:grid">
              <Icon name="zoom_in" size={16} />
            </span>
          </button>
        ))}
      </div>
      {open !== null && images[open] ? <Lightbox images={images} index={open} onChange={setOpen} onClose={() => setOpen(null)} /> : null}
    </>
  );
}

function Lightbox(props: { images: PreviewImage[]; index: number; onChange: (index: number) => void; onClose: () => void }) {
  const { images, index, onChange, onClose } = props;
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key === 'Escape') onClose();
      if (event.key === 'ArrowRight') onChange((index + 1) % images.length);
      if (event.key === 'ArrowLeft') onChange((index - 1 + images.length) % images.length);
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [images.length, index, onChange, onClose]);
  const image = images[index];
  return createPortal(
    <div className="fixed inset-0 z-[80] grid place-items-center bg-black/72 p-6" role="dialog" aria-label="截图预览" onClick={onClose}>
      <img
        src={image.url}
        alt={image.alt}
        referrerPolicy="no-referrer"
        className="max-h-[88vh] max-w-[92vw] rounded-lg bg-white object-contain shadow-2xl"
        onClick={(event) => event.stopPropagation()}
      />
      <button type="button" className="absolute top-4 right-4 grid size-9 place-items-center rounded-full bg-white/15 text-white hover:bg-white/25" onClick={onClose} aria-label="关闭预览">
        <Icon name="close" size={20} />
      </button>
      {images.length > 1 ? (
        <>
          <button
            type="button"
            className="absolute top-1/2 left-4 grid size-10 -translate-y-1/2 place-items-center rounded-full bg-white/15 text-white hover:bg-white/25"
            onClick={(event) => { event.stopPropagation(); onChange((index - 1 + images.length) % images.length); }}
            aria-label="上一张"
          >
            <Icon name="chevron_left" size={24} />
          </button>
          <button
            type="button"
            className="absolute top-1/2 right-4 grid size-10 -translate-y-1/2 place-items-center rounded-full bg-white/15 text-white hover:bg-white/25"
            onClick={(event) => { event.stopPropagation(); onChange((index + 1) % images.length); }}
            aria-label="下一张"
          >
            <Icon name="chevron_right" size={24} />
          </button>
          <span className="absolute bottom-4 left-1/2 -translate-x-1/2 rounded-full bg-black/55 px-3 py-1 text-xs text-white tabular-nums">{index + 1} / {images.length}</span>
        </>
      ) : null}
    </div>,
    document.body,
  );
}

/* ─────────────── 项目说明原文 ─────────────── */

export function ReadmePanel(props: { detail: LibraryDetail; readme: ReadmeState }) {
  if (props.readme.state === 'loading') return <PanelSkeleton />;
  if (props.readme.state === 'error') return panelError(props.readme.message);
  if (!props.readme.readme) {
    return (
      <EmptyState
        icon="description"
        title={README_NOT_DOWNLOADED_TITLE}
        description={README_DOWNLOAD_HOW}
      />
    );
  }
  return (
    <ReadmeRenderer
      markdown={props.readme.readme.rawMarkdown}
      repositoryFullName={props.detail.fullName}
      sourcePath={props.readme.readme.sourcePath}
      className="readme-rendered min-w-0 text-sm text-on-surface"
    />
  );
}

/* ─────────────── 社区：别人怎么用 ─────────────── */

export function CommunityPanel(props: { detail: LibraryDetail; scenarioName?: string | null; onCopyPrompt: (text: string, kind: CopyKind) => void }) {
  const links = communityLinks(props.detail.fullName);
  return (
    <div className="flex flex-col gap-3">
      <CommunityDigest repoId={props.detail.id} />
      <SoftCard className="flex items-start gap-3">
        <span className="hue-tile grid size-10 shrink-0 place-items-center rounded-[10px]" style={{ '--h': 268 } as React.CSSProperties}>
          <Icon name="travel_explore" size={22} fill />
        </span>
        <span className="min-w-0 flex-1">
          <b className="block text-sm font-semibold text-foreground">让 Codex 去查别人怎么用</b>
          <span className="block text-xs leading-[1.6] text-on-surface-variant">
            复制一段话粘给 Codex：它会用你的 web-search，在小红书、知乎、B 站、X、Reddit、YouTube 等地方找真实的使用案例、踩过的坑和替代品，只引用查到的内容，附原帖链接。
          </span>
          <Button className="mt-2.5" onClick={() => props.onCopyPrompt(buildCommunityPrompt({ fullName: props.detail.fullName, htmlUrl: props.detail.htmlUrl, form: props.detail.form, scenarioName: props.scenarioName }), 'community')}>
            <Icon name="content_copy" size={16} />
            复制给 Codex
          </Button>
        </span>
      </SoftCard>

      <div>
        <SectionTitle icon="open_in_new" hue={222}>自己去看</SectionTitle>
        <div className="grid grid-cols-2 gap-2">
          {links.map((link) => (
            <a
              key={link.id}
              href={link.url}
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-2.5 rounded-lg border border-border bg-card px-3 py-2.5 transition-colors hover:border-input hover:bg-surface-container-low"
            >
              <Icon name={link.icon} size={18} className="shrink-0 text-on-surface-variant" />
              <span className="min-w-0">
                <b className="block truncate text-[13px] font-medium text-foreground">{link.label}</b>
                <span className="block truncate text-xs text-on-surface-variant">{link.hint}</span>
              </span>
            </a>
          ))}
        </div>
        <p className="mt-2.5 text-xs leading-[1.6] text-on-surface-variant">这些链接会在浏览器里打开，用的是你自己的登录状态。小红书、知乎、X 要登录才能看，所以只能交给你自己或 Codex。</p>
      </div>
    </div>
  );
}

/** 内置的讨论摘要：Hacker News 上的帖子 + GitHub 上评论最多的 Issues。平时只读本地缓存，点「查一查」才联网。 */
function CommunityDigest(props: { repoId: string }) {
  const { state, refresh } = useRepoCommunity(props.repoId);
  if (state.phase === 'unavailable') return null;
  const now = new Date();
  const data = state.data;
  const groups = data ? groupCommunity(data.items) : [];
  const note = communityNote(data, now);
  const checked = Boolean(data?.fetchedAt);

  return (
    <div>
      <div className="mb-2 flex items-center gap-2">
        <h3 className="m-0 flex min-w-0 flex-1 items-center gap-2 text-[13px] font-semibold text-foreground">
          <span className="hue-tile grid size-6 place-items-center rounded-md" style={{ '--h': 28 } as React.CSSProperties}>
            <Icon name="forum" size={15} fill />
          </span>
          社区里在聊什么
        </h3>
        {state.phase !== 'loading' ? (
          <Button size="sm" loading={state.refreshing} onClick={refresh}>
            {checked ? '重新查' : '查一查'}
          </Button>
        ) : null}
      </div>

      {state.phase === 'loading' ? <div className="flex items-center gap-2 text-xs text-on-surface-variant"><Spinner /> 读取中…</div> : null}
      {state.refreshing ? (
        <div className="mb-2 flex items-center gap-2 rounded-lg bg-accent px-3 py-2 text-xs text-foreground">
          <Spinner /> 查英文技术社区 Hacker News 和这个项目的「问题反馈区」（GitHub Issues）
        </div>
      ) : null}
      {state.error ? <div className="mb-2 rounded-lg bg-error-container px-3 py-2 text-xs text-on-error-container">{state.error}</div> : null}
      {!state.refreshing && note ? (
        <p className={cn('mt-0 mb-2 text-xs leading-[1.6]', note.tone === 'warn' ? 'text-error' : 'text-on-surface-variant')}>{note.text}</p>
      ) : null}

      {state.phase === 'ready' && !checked && !state.refreshing ? (
        <SoftCard className="text-xs leading-[1.7] text-on-surface-variant">
          还没查过。点「查一查」，会查英文技术社区 Hacker News 和这个项目的「问题反馈区」（GitHub Issues），按热度列在这里：别人拿它做了什么、踩过什么坑，一眼就能扫一遍。
        </SoftCard>
      ) : null}

      {groups.map((group) => (
        <div key={group.source} className="mb-3 last:mb-0">
          <div className="mb-1 text-xs font-medium text-on-surface-variant">{group.label}</div>
          <ul className="m-0 flex list-none flex-col gap-1.5 p-0">
            {group.items.map((item) => {
              const meta = itemMeta(item);
              const status = stateLabel(item);
              return (
                <li key={item.url}>
                  <a
                    href={item.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="flex items-start gap-3 rounded-lg border border-border bg-card px-3 py-2.5 transition-colors hover:border-input hover:bg-surface-container-low"
                  >
                    <span className="min-w-0 flex-1">
                      <span className="line-clamp-2 text-[13px] leading-[1.5] font-medium text-foreground">{item.title}</span>
                      <span className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs text-on-surface-variant">
                        {meta ? <span className="tabular-nums">{meta}</span> : null}
                        {item.createdAt ? <span>{relativeDays(item.createdAt, now)}</span> : null}
                        {status ? <Tag tone={item.state === 'open' ? 'blue' : 'gray'}>{status}</Tag> : null}
                      </span>
                    </span>
                    <Icon name="open_in_new" size={14} className="mt-0.5 shrink-0 text-on-surface-variant" />
                  </a>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
    </div>
  );
}
