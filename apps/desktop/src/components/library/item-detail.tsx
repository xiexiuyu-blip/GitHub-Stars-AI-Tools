import type { LibraryDetail, ReadingGroup, ScenarioSummary } from '@/lib/contracts';
import { cleanHowToTry, clipText, firstSentences, formatStars, monthDay, relativeDays, splitRepoName } from '@/lib/item-format';
import { Button, EmptyState, RepoAvatar, Tag } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { ReadingSegmented } from './reading-segmented';

const HOW_EMPTY = 'README 里没有现成的安装步骤。点「打开 GitHub」看 Releases 或文档；也可以直接复制给 Codex，让它去读 README。';

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
  onCopyCodex: () => void;
  onCopyHow: (text: string) => void;
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
          description="右边会出现它的中文说明、怎么试，和复制给 Codex 的按钮"
        />
      </div>
    );
  }

  const detail = props.detail;
  const { owner, repo } = splitRepoName(detail.fullName);
  const how = cleanHowToTry(detail.howToTry);
  const summarySource = detail.whatItIs?.trim() ? detail.whatItIs : detail.oneLiner;
  const capabilities = detail.capabilities.slice(0, 4).map((item) => clipText(item, 46));
  const scenarioNames = detail.scenarioIds
    .map((id) => props.scenarios?.find((scenario) => scenario.id === id)?.nameZh?.trim())
    .filter((name): name is string => Boolean(name));
  const topics = detail.topics.map((topic) => topic.trim()).filter(Boolean);
  const now = new Date();

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="min-h-0 flex-1 overflow-auto px-6 py-5">
        <div className="flex items-start gap-3">
          <RepoAvatar name={detail.fullName} size="lg" />
          <div className="min-w-0">
            <h2 className="m-0 text-[20px] leading-[1.4] font-semibold break-words text-foreground">{repo || detail.fullName}</h2>
            <div className="mt-1 flex flex-wrap items-center gap-x-1.5 gap-y-1 text-xs text-on-surface-variant">
              {owner ? <span>{owner}</span> : null}
              <span>{owner ? '· ' : ''}★ {formatStars(detail.starsCount)}</span>
              {detail.language ? <span>· {detail.language}</span> : null}
              <span>· 最近更新 {relativeDays(detail.pushedAt, now)}</span>
              <span>· 收藏于 {monthDay(detail.starredAt)}</span>
              {detail.form ? <Tag>{detail.form}</Tag> : null}
              {detail.maturity ? <Tag>{detail.maturity}</Tag> : null}
              {detail.zhFriendly === true ? <Tag>中文友好</Tag> : null}
            </div>
          </div>
        </div>

        <ReadingSegmented className="mt-3.5" value={detail.readingGroup} onChange={props.onReadingChange} />

        {detail.hasCard ? (
          <div className="mt-4 rounded-lg bg-accent px-3.5 py-3">
            <div className="flex items-center gap-1.5 text-xs font-medium text-accent-foreground">
              <Icon name="auto_awesome" size={14} />
              AI 说明
              <span className="ml-auto font-normal text-on-surface-variant">依据 README</span>
            </div>
            <p className="mt-1.5 text-sm leading-[1.75] text-foreground">{firstSentences(summarySource, 3)}</p>
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
          <div className="mt-4 rounded-lg bg-accent px-3.5 py-3">
            <div className="flex items-center gap-1.5 text-xs font-medium text-accent-foreground">
              还没有中文说明
            </div>
            <p className="mt-1.5 text-sm text-on-surface-variant">整理好之后，这里会出现中文说明、能做什么、怎么试。现在先看 GitHub 上的原文：</p>
            <p className="mt-1.5 text-sm leading-[1.75] text-foreground">{detail.description?.trim() || '—'}</p>
          </div>
        )}

        {capabilities.length > 0 ? (
          <section className="mt-5">
            <h3 className="mb-1.5 text-xs text-on-surface-variant">它能做</h3>
            <ul className="mt-1.5 list-disc pl-[18px] text-sm leading-[1.75] text-foreground">
              {capabilities.map((item, index) => <li key={`${index}-${item}`}>{item}</li>)}
            </ul>
          </section>
        ) : null}

        {detail.problemsSolved.length > 0 ? (
          <section className="mt-5">
            <h3 className="mb-1.5 text-xs text-on-surface-variant">适合解决</h3>
            <ul className="mt-1.5 list-disc pl-[18px] text-sm leading-[1.75] text-foreground">
              {detail.problemsSolved.map((item, index) => <li key={`${index}-${item}`}>{item}</li>)}
            </ul>
          </section>
        ) : null}

        {detail.hasCard ? (
          <section className="mt-5">
            <h3 className="mb-1.5 text-xs text-on-surface-variant">怎么试</h3>
            {how ? (
              <pre className="relative mt-0.5 rounded-lg border border-border bg-background px-3 py-2.5 pr-10 font-mono text-xs leading-[1.7] break-words whitespace-pre-wrap text-foreground">
                {how}
                <button
                  type="button"
                  className="absolute top-1.5 right-1.5 inline-grid size-7 place-items-center rounded-md text-on-surface-variant hover:bg-surface-container-low hover:text-foreground"
                  title="复制"
                  aria-label="复制安装步骤"
                  onClick={() => props.onCopyHow(how)}
                >
                  <Icon name="content_copy" size={16} />
                </button>
              </pre>
            ) : (
              <p className="text-sm text-on-surface-variant">{HOW_EMPTY}</p>
            )}
          </section>
        ) : null}

        <details className="group mt-5">
          <summary className="flex cursor-pointer list-none items-center gap-1 text-xs text-on-surface-variant [&::-webkit-details-marker]:hidden">
            <Icon name="chevron_right" size={16} className="transition-transform group-open:rotate-90" />
            更多信息
          </summary>
          <section className="mt-2">
            <h3 className="mb-1.5 text-xs text-on-surface-variant">GitHub 原文描述</h3>
            <p className="text-sm text-on-surface-variant">{detail.description?.trim() || '—'}</p>
          </section>
          {topics.length > 0 ? (
            <section className="mt-5">
              <h3 className="mb-1.5 text-xs text-on-surface-variant">话题标签</h3>
              <div className="flex flex-wrap gap-1.5">
                {topics.map((topic) => <Tag key={topic}>{topic}</Tag>)}
              </div>
            </section>
          ) : null}
          {scenarioNames.length > 0 ? (
            <section className="mt-5">
              <h3 className="mb-1.5 text-xs text-on-surface-variant">适合的场景</h3>
              <div className="flex flex-wrap gap-1.5">
                {scenarioNames.map((name) => <Tag key={name} tone="blue">{name}</Tag>)}
              </div>
            </section>
          ) : null}
        </details>
      </div>

      <div className="flex shrink-0 flex-wrap items-center gap-2 border-t border-muted px-4 py-3">
        {props.onBack ? (
          <Button variant="text" onClick={props.onBack}>
            <Icon name="arrow_back" size={16} />
            返回
          </Button>
        ) : null}
        {detail.htmlUrl ? (
          <a
            className="inline-flex h-7 items-center gap-1 px-1 text-sm text-accent-foreground hover:underline hover:underline-offset-[3px]"
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
    <div className="flex flex-1 flex-col gap-3 p-6" aria-hidden="true">
      <div className="h-10 w-2/3 animate-pulse rounded-md bg-muted" />
      <div className="h-4 w-1/2 animate-pulse rounded-md bg-muted" />
      <div className="mt-2 h-24 animate-pulse rounded-lg bg-muted" />
      <div className="h-4 w-full animate-pulse rounded-md bg-muted" />
      <div className="h-4 w-5/6 animate-pulse rounded-md bg-muted" />
    </div>
  );
}
