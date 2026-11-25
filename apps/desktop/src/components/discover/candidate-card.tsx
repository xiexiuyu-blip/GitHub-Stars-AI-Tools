import type { DiscoverCardItem } from '@/lib/contracts';
import type { CandidateStatus } from '@/lib/discover-view';
import { COPY_EVALUATE_BUTTON, relativeDays, splitRepoName } from '@/lib/item-format';
import { Button, LangDot, RepoAvatar, StarCount, Tag } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';

const MAX_TOPICS = 3;

/**
 * 「发现」里的一个候选项目：支持场景发现候选、趋势榜（含排名与当日/本周增量）、HN Show 与自由搜索结果。
 */
export function CandidateCard(props: {
  candidate: DiscoverCardItem;
  status: CandidateStatus;
  adding?: boolean;
  now: Date;
  onAdd: () => void;
  onCopyEvaluate: () => void;
  onOpenInbox?: () => void;
}) {
  const { candidate, status } = props;
  const { owner, repo } = splitRepoName(candidate.fullName);
  const description = candidate.description?.trim();
  const topics = candidate.topics.slice(0, candidate.sourceNote ? 2 : MAX_TOPICS);
  const showStars = candidate.starsCount > 0 || !candidate.deltaLabel;
  const rank = candidate.rank;

  return (
    <article className="flex min-w-0 flex-col gap-2.5 rounded-lg border border-border bg-card p-4 shadow-xs transition-shadow hover:shadow-md">
      <div className="flex min-w-0 items-start gap-3">
        {typeof rank === 'number' && rank > 0 ? (
          <span
            className={cn(
              'grid size-7 shrink-0 place-items-center rounded-md text-xs font-semibold tabular-nums',
              rank <= 3 ? 'bg-primary/15 text-primary' : 'bg-surface-container-high text-on-surface-variant',
            )}
          >
            #{rank}
          </span>
        ) : null}
        <RepoAvatar name={candidate.fullName} size="md" />
        <div className="min-w-0 flex-1">
          <a
            href={candidate.htmlUrl}
            target="_blank"
            rel="noreferrer"
            className="block truncate text-[15px] leading-[1.35] font-semibold text-foreground hover:text-primary"
            title={`在 GitHub 打开 ${candidate.fullName}`}
          >
            {repo || candidate.fullName}
          </a>
          <div className="mt-0.5 flex min-w-0 items-center gap-2 text-xs text-subtle">
            {owner ? <span className="min-w-0 truncate">{owner}</span> : null}
            <LangDot language={candidate.language} />
          </div>
        </div>
        <div className="flex shrink-0 flex-col items-end gap-1">
          {showStars ? <StarCount value={candidate.starsCount} className="shrink-0" /> : null}
          {candidate.deltaLabel ? (
            <span className="inline-flex shrink-0 items-center rounded-md bg-primary/10 px-1.5 py-0.5 text-[11px] font-medium text-primary tabular-nums">
              {candidate.deltaLabel}
            </span>
          ) : null}
        </div>
      </div>

      <p className="m-0 line-clamp-3 min-h-[3.9em] text-sm leading-[1.55] text-foreground">
        {description || '（作者没有写简介）'}
      </p>

      <div className="flex min-w-0 flex-wrap items-center gap-1.5 text-xs text-on-surface-variant">
        {candidate.sourceNote ? <Tag>{candidate.sourceNote}</Tag> : null}
        {topics.map((topic) => (
          <Tag key={topic}>{topic}</Tag>
        ))}
        {candidate.createdAt && !candidate.pushedAt ? (
          <span className="ml-auto shrink-0">新建于：{relativeDays(candidate.createdAt, props.now)}</span>
        ) : candidate.pushedAt ? (
          <span className="ml-auto shrink-0">项目最近更新：{relativeDays(candidate.pushedAt, props.now)}</span>
        ) : null}
      </div>

      <div className="mt-auto flex flex-wrap items-center gap-2 border-t border-muted pt-2.5">
        {status === 'mine' ? (
          <span className="inline-flex items-center gap-1 text-[13px] text-on-surface-variant">
            <Icon name="check_circle" size={16} fill className="text-[hsl(148_55%_42%)]" />
            你已经收藏了
          </span>
        ) : status === 'inbox' ? (
          <span className="inline-flex items-center gap-1 text-[13px] text-on-surface-variant">
            <Icon name="inbox" size={16} fill className="text-primary" />
            已在收集箱
            {props.onOpenInbox ? (
              <Button variant="text" size="sm" className="h-6 text-xs" onClick={props.onOpenInbox}>去看看</Button>
            ) : null}
          </span>
        ) : (
          <Button size="sm" loading={props.adding} onClick={props.onAdd}>
            <Icon name="add" size={16} />
            先放进收集箱
          </Button>
        )}
        <Button variant="text" size="sm" className="ml-auto" onClick={props.onCopyEvaluate}>
          {COPY_EVALUATE_BUTTON}
        </Button>
        <a
          href={candidate.htmlUrl}
          target="_blank"
          rel="noreferrer"
          className="inline-flex h-7 items-center gap-0.5 rounded-md px-1 text-sm text-accent-foreground hover:underline hover:underline-offset-[3px]"
        >
          GitHub
          <Icon name="open_in_new" size={14} />
        </a>
      </div>
    </article>
  );
}
