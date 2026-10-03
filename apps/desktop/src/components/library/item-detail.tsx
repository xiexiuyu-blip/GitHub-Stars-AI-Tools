import { useMemo, useState } from 'react';
import type { LibraryDetail, ReadingGroup, ScenarioSummary } from '@/lib/contracts';
import { cleanHowToTry, clipText, firstSentences, monthDay, relativeDays, splitRepoName } from '@/lib/item-format';
import { explainerLinks, extractInstallSteps, extractPreviewImages, hasDownloadHint } from '@/lib/repo-assets';
import { itemPendingNote } from '@/lib/automation-view';
import { formLabel } from '@/lib/library-filters';
import { useAiAutomation } from '@/hooks/use-ai-automation';
import { useRepoReadme } from '@/hooks/use-repo-readme';
import { Button, CoverImage, EmptyState, LangDot, RepoAvatar, ScenarioChip, StarCount, Tag } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';
import { CommunityPanel, InstallPanel, PreviewPanel, ReadmePanel, SectionTitle, SoftCard } from './detail-panels';
import { ReadingSegmented } from './reading-segmented';

const HOW_EMPTY = 'README 里没有现成的安装步骤。点「打开 GitHub」看 Releases 或文档；也可以直接复制给 Codex，让它去读 README。';

type TabKey = 'overview' | 'install' | 'preview' | 'readme' | 'community';

/** 记住上次停在哪一页：在「预览」里用 J/K 连着翻几个项目时，不用每次再点一遍。 */
let lastTab: TabKey = 'overview';

export function ItemDetail(props: {
  detail: LibraryDetail | null;
  loading?: boolean;
  error?: string | null;
  scenarios?: ScenarioSummary[];
  onBack?: () => void;
  onRetry?: () => void;
  onReadingChange: (group: ReadingGroup) => void;
  onConfirm: () => void;
  onRegenerate: () => void;
  /** 去工具箱（开启 / 调整 AI 自动整理）。不传就不画这个按钮。 */
  onOpenToolbox?: () => void;
  onCopyCodex: () => void;
  onCopyHow: (text: string) => void;
  /** 复制一段「给 Codex 的话」（安装请求、查别人怎么用）。不传就用 onCopyHow。 */
  onCopyPrompt?: (text: string) => void;
}) {
  if (!props.detail) {
    if (props.loading) return <DetailSkeleton />;
    if (props.error) {
      return (
        <div className="flex flex-1 items-center justify-center">
          <EmptyState
            icon="error"
            title="没有读出这个项目"
            description={props.error}
            action={props.onRetry ? <Button variant="text" onClick={props.onRetry}>重试</Button> : undefined}
          />
        </div>
      );
    }
    return (
      <div className="flex flex-1 items-center justify-center">
        <EmptyState
          icon="ads_click"
          title="选一个工具"
          description="右边会出现它的中文说明、怎么装、截图预览，和复制给 Codex 的按钮"
        />
      </div>
    );
  }
  // key：换一个仓库就重新开始（重新读 README；停在哪一页会记住）
  return <DetailBody key={props.detail.id} {...props} detail={props.detail} />;
}

function DetailBody(props: Parameters<typeof ItemDetail>[0] & { detail: LibraryDetail }) {
  const { detail } = props;
  const [tab, setTabState] = useState<TabKey>(lastTab);
  const setTab = (next: TabKey) => {
    lastTab = next;
    setTabState(next);
  };
  const readme = useRepoReadme(detail.id);
  const markdown = readme.state === 'ready' ? readme.readme?.rawMarkdown ?? '' : '';
  const images = useMemo(() => extractPreviewImages(markdown, detail.fullName, 12), [markdown, detail.fullName]);
  const steps = useMemo(() => extractInstallSteps(markdown, 3), [markdown]);
  const downloadHint = useMemo(() => hasDownloadHint(markdown), [markdown]);
  const cover = images[0]?.url ?? null;

  const { owner, repo } = splitRepoName(detail.fullName);
  const how = cleanHowToTry(detail.howToTry);
  const summarySource = detail.whatItIs?.trim() ? detail.whatItIs : detail.oneLiner;
  const capabilities = detail.capabilities.slice(0, 4).map((item) => clipText(item, 46));
  const scenarioList = detail.scenarioIds
    .map((id) => props.scenarios?.find((scenario) => scenario.id === id))
    .filter((scenario): scenario is ScenarioSummary => Boolean(scenario));
  const topics = detail.topics.map((topic) => topic.trim()).filter(Boolean);
  const now = new Date();
  const copyPrompt = props.onCopyPrompt ?? props.onCopyHow;

  const tabs: Array<{ key: TabKey; label: string; icon: string; count?: number }> = [
    { key: 'overview', label: '概览', icon: 'dashboard' },
    { key: 'install', label: '安装', icon: 'terminal', count: steps.length || undefined },
    { key: 'preview', label: '预览', icon: 'image', count: images.length || undefined },
    { key: 'readme', label: 'README', icon: 'description' },
    { key: 'community', label: '社区', icon: 'forum' },
  ];

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="min-h-0 flex-1 overflow-auto">
        {/* 封面：README 里的第一张截图，完整显示（四周用它自己模糊的样子垫底）；没有图就只是一条按名字上色的渐变，不放大字母 */}
        <CoverImage
          src={cover}
          seed={detail.fullName}
          fit="contain"
          letter={false}
          className={cn('w-full shrink-0 border-b border-border', cover ? 'h-[200px]' : 'h-[72px]')}
        />

        <div className="px-6 pb-6">
          {/* 头像压在封面下沿上（只有头像重叠，标题完整落在封面下面） */}
          <div className="relative z-10 grid grid-cols-[60px_minmax(0,1fr)] items-start gap-3">
            <RepoAvatar name={detail.fullName} size="lg" className="-mt-7 ml-1 shadow-md ring-4 ring-card" />
            <div className="min-w-0 pt-2.5">
              <h2 className="m-0 text-[22px] leading-[1.25] font-bold tracking-tight break-words text-foreground">{repo || detail.fullName}</h2>
              {owner ? <div className="mt-0.5 text-[13px] text-on-surface-variant">{owner}</div> : null}
            </div>
          </div>

          <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5">
            <StarCount value={detail.starsCount} className="text-[13px]" />
            <LangDot language={detail.language} className="text-[13px]" />
            <span className="text-xs text-on-surface-variant">最近更新 {relativeDays(detail.pushedAt, now)}</span>
            <span className="text-xs text-on-surface-variant">收藏于 {monthDay(detail.starredAt)}</span>
          </div>
          <div className="mt-2.5 flex flex-wrap items-center gap-1.5">
            {scenarioList.map((scenario) => (
              <ScenarioChip key={scenario.id} name={scenario.nameZh} scenarioKey={scenario.slug || scenario.id} />
            ))}
            {formLabel(detail.form) ? <Tag>{formLabel(detail.form)}</Tag> : null}
            {detail.maturity ? <Tag>{detail.maturity}</Tag> : null}
            {detail.zhFriendly === true ? <Tag tone="blue">中文友好</Tag> : null}
          </div>

          <ReadingSegmented className="mt-4" value={detail.readingGroup} onChange={props.onReadingChange} />

          {detail.hasCard ? (
            <div className="mt-4 rounded-lg bg-accent px-4 py-3.5">
              <div className="flex items-center gap-1.5 text-xs font-medium text-accent-foreground">
                <Icon name="auto_awesome" size={14} fill />
                AI 说明
                <span className="ml-auto font-normal text-on-surface-variant">依据 README</span>
              </div>
              <p className="mt-1.5 text-sm leading-[1.8] text-foreground">{firstSentences(summarySource, 3)}</p>
              <div className="mt-2 flex flex-wrap items-center gap-0.5 text-xs text-on-surface-variant">
                {detail.cardReview === 'confirmed' ? (
                  <span className="inline-flex items-center gap-1">
                    <Icon name="check" size={14} />
                    你确认过了
                  </span>
                ) : (
                  <>
                    说得对吗？
                    <Button variant="text" size="sm" className="h-6 px-1 text-xs" onClick={props.onConfirm}>对</Button>
                    <Button variant="text" size="sm" className="h-6 px-1 text-xs" onClick={props.onRegenerate}>不对，重新整理</Button>
                  </>
                )}
              </div>
            </div>
          ) : (
            <PendingNote fullName={detail.fullName} description={detail.description} onOpenToolbox={props.onOpenToolbox} />
          )}

          <div role="tablist" aria-label="项目详情" className="mt-5 flex gap-1 border-b border-border">
            {tabs.map((item) => (
              <button
                key={item.key}
                type="button"
                role="tab"
                aria-selected={tab === item.key}
                onClick={() => setTab(item.key)}
                className={cn(
                  '-mb-px inline-flex h-9 items-center gap-1.5 border-b-2 px-2.5 text-[13px] whitespace-nowrap transition-colors',
                  tab === item.key
                    ? 'border-primary font-semibold text-primary'
                    : 'border-transparent text-on-surface-variant hover:text-foreground',
                )}
              >
                <Icon name={item.icon} size={16} fill={tab === item.key} />
                {item.label}
                {item.count ? (
                  <span className={cn('rounded-full px-1.5 text-[11px] leading-[16px] tabular-nums', tab === item.key ? 'bg-accent text-accent-foreground' : 'bg-muted text-on-surface-variant')}>{item.count}</span>
                ) : null}
              </button>
            ))}
          </div>

          <div className="mt-4" role="tabpanel">
            {tab === 'overview' ? (
              <div className="flex flex-col gap-3">
                {capabilities.length > 0 ? (
                  <SoftCard>
                    <SectionTitle icon="bolt" hue={222}>它能做</SectionTitle>
                    <ul className="m-0 flex list-none flex-col gap-1.5 p-0">
                      {capabilities.map((item, index) => (
                        <li key={`${index}-${item}`} className="flex items-start gap-2 text-sm leading-[1.6] text-foreground">
                          <Icon name="check_circle" size={16} fill className="mt-[3px] shrink-0 text-[hsl(148_55%_42%)]" />
                          <span className="min-w-0">{item}</span>
                        </li>
                      ))}
                    </ul>
                  </SoftCard>
                ) : null}

                {detail.problemsSolved.length > 0 ? (
                  <SoftCard>
                    <SectionTitle icon="flag" hue={28}>适合解决</SectionTitle>
                    <ul className="m-0 flex list-none flex-col gap-1.5 p-0">
                      {detail.problemsSolved.map((item, index) => (
                        <li key={`${index}-${item}`} className="flex items-start gap-2 text-sm leading-[1.6] text-foreground">
                          <Icon name="arrow_right" size={18} className="mt-0.5 shrink-0 text-[hsl(28_85%_50%)]" />
                          <span className="min-w-0">{item}</span>
                        </li>
                      ))}
                    </ul>
                  </SoftCard>
                ) : null}

                {detail.hasCard ? (
                  <SoftCard>
                    <SectionTitle icon="play_circle" hue={148}>怎么试</SectionTitle>
                    {how ? (
                      <pre className="code-block relative m-0 rounded-lg px-3 py-2.5 pr-11 text-xs leading-[1.7] break-words whitespace-pre-wrap">
                        {how}
                        <button
                          type="button"
                          className="absolute top-1.5 right-1.5 inline-grid size-7 place-items-center rounded-md text-white/70 hover:bg-white/10 hover:text-white"
                          title="复制"
                          aria-label="复制安装步骤"
                          onClick={() => props.onCopyHow(how)}
                        >
                          <Icon name="content_copy" size={16} />
                        </button>
                      </pre>
                    ) : (
                      <p className="m-0 text-sm leading-[1.6] text-on-surface-variant">{HOW_EMPTY}</p>
                    )}
                    <button type="button" className="mt-2 text-xs text-accent-foreground hover:underline" onClick={() => setTab('install')}>
                      看完整的安装步骤 →
                    </button>
                  </SoftCard>
                ) : null}

                <details className="group">
                  <summary className="flex cursor-pointer list-none items-center gap-1 text-xs text-on-surface-variant [&::-webkit-details-marker]:hidden">
                    <Icon name="chevron_right" size={16} className="transition-transform group-open:rotate-90" />
                    更多信息
                  </summary>
                  <section className="mt-2">
                    <h3 className="mb-1.5 text-xs text-on-surface-variant">GitHub 原文描述</h3>
                    <p className="text-sm text-on-surface-variant">{detail.description?.trim() || '—'}</p>
                  </section>
                  {topics.length > 0 ? (
                    <section className="mt-4">
                      <h3 className="mb-1.5 text-xs text-on-surface-variant">话题标签</h3>
                      <div className="flex flex-wrap gap-1.5">
                        {topics.map((topic) => <Tag key={topic}>{topic}</Tag>)}
                      </div>
                    </section>
                  ) : null}
                </details>
              </div>
            ) : null}

            {tab === 'install' ? (
              <InstallPanel
                detail={detail}
                readme={readme}
                steps={steps}
                howFallback={how}
                hasDownload={downloadHint}
                scenarioName={scenarioList[0]?.nameZh ?? null}
                onCopyText={props.onCopyHow}
                onCopyPrompt={copyPrompt}
              />
            ) : null}
            {tab === 'preview' ? <PreviewPanel detail={detail} readme={readme} images={images} /> : null}
            {tab === 'readme' ? <ReadmePanel detail={detail} readme={readme} /> : null}
            {tab === 'community' ? <CommunityPanel detail={detail} scenarioName={scenarioList[0]?.nameZh ?? null} onCopyPrompt={copyPrompt} /> : null}
          </div>
        </div>
      </div>

      <div className="flex shrink-0 flex-wrap items-center gap-2 border-t border-border bg-card px-4 py-3">
        {props.onBack ? (
          <Button variant="text" onClick={props.onBack}>
            <Icon name="arrow_back" size={16} />
            返回
          </Button>
        ) : null}
        {detail.htmlUrl ? (
          <a
            className="inline-flex h-8 items-center gap-1 rounded-md border border-input bg-card px-3 text-sm text-foreground hover:bg-surface-container-low"
            href={detail.htmlUrl}
            target="_blank"
            rel="noopener noreferrer"
          >
            打开 GitHub
            <Icon name="open_in_new" size={14} />
          </a>
        ) : null}
        <span className="flex-1" />
        {/* 页面主按钮：全页只此一个蓝色实底按钮 */}
        <Button variant="primary" onClick={props.onCopyCodex}>
          <Icon name="content_copy" size={16} />
          复制给 Codex
        </Button>
      </div>
    </div>
  );
}

function DetailSkeleton() {
  return (
    <div className="flex flex-1 flex-col" aria-hidden="true">
      <div className="h-[150px] animate-pulse bg-muted" />
      <div className="flex flex-col gap-3 p-6">
        <div className="h-8 w-2/3 animate-pulse rounded-md bg-muted" />
        <div className="h-4 w-1/2 animate-pulse rounded-md bg-muted" />
        <div className="mt-2 h-24 animate-pulse rounded-lg bg-muted" />
        <div className="h-4 w-full animate-pulse rounded-md bg-muted" />
        <div className="h-4 w-5/6 animate-pulse rounded-md bg-muted" />
      </div>
    </div>
  );
}

/** 还没有中文说明：说清为什么（没开启 / 暂停 / 额度用完 / 在排队），需要你动手时给一个去工具箱的入口，下面先显示 GitHub 原文。 */
function PendingNote(props: { fullName: string; description: string | null | undefined; onOpenToolbox?: () => void }) {
  const { status } = useAiAutomation();
  const note = itemPendingNote(status);
  const explainers = explainerLinks(props.fullName);
  return (
    <div className="mt-4 rounded-lg bg-accent px-4 py-3.5">
      <div className="flex items-center gap-1.5 text-xs font-medium text-accent-foreground">
        <Icon name="hourglass_top" size={14} fill />
        还没有中文说明
        {note.actionLabel && props.onOpenToolbox ? (
          <Button variant="text" size="sm" className="ml-auto h-6 px-1 text-xs" onClick={props.onOpenToolbox}>{note.actionLabel}</Button>
        ) : null}
      </div>
      <p className="mt-1.5 text-sm text-on-surface-variant">{note.text}</p>
      <p className="mt-1.5 text-[15px] leading-[1.75] text-foreground">{props.description?.trim() || '—'}</p>
      {explainers.length > 0 ? (
        <div className="mt-2.5 flex flex-wrap items-center gap-x-3 gap-y-1 border-t border-border/60 pt-2.5 text-xs text-on-surface-variant">
          <span>想先了解它？看别人生成的解读：</span>
          {explainers.map((link) => (
            <a key={link.id} href={link.url} target="_blank" rel="noopener noreferrer" title={link.hint} className="inline-flex items-center gap-0.5 text-accent-foreground hover:underline hover:underline-offset-[3px]">
              {link.label}
              <Icon name="open_in_new" size={13} />
            </a>
          ))}
        </div>
      ) : null}
    </div>
  );
}
