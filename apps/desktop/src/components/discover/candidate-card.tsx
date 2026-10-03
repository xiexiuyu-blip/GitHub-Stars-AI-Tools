import type { DiscoverCandidate } from '@/lib/contracts';
import type { CandidateStatus } from '@/lib/discover-view';
import { relativeDays, splitRepoName } from '@/lib/item-format';
import { Button, LangDot, RepoAvatar, StarCount, Tag } from '@/components/kit';
import { Icon } from '@/components/ui/icon';

const MAX_TOPICS = 3;

/**
 * 「发现」里的一个候选项目：还没收藏的 GitHub 项目。
 * 描述是 GitHub 上的原文（英文居多），所以按钮里有「复制给 Codex 评估」，让 Codex 读完用中文讲。
 */
export function CandidateCard(props: {
  candidate: DiscoverCandidate;
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
  const topics = candidate.topics.slice(0, MAX_TOPICS);

  return (
    <article className="flex min-w-0 flex-col gap-2.5 rounded-lg border border-border bg-card p-4 shadow-xs transition-shadow hover:shadow-md">
      <div className="flex min-w-0 items-start gap-3">
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
        <StarCount value={candidate.starsCount} className="shrink-0" />
      </div>

      <p className="m-0 line-clamp-3 min-h-[3.9em] text-sm leading-[1.55] text-foreground">
        {description || '（作者没有写简介）'}
      </p>

      <div className="flex min-w-0 flex-wrap items-center gap-1.5 text-xs text-on-surface-variant">
        {topics.map((topic) => (
          <Tag key={topic}>{topic}</Tag>
        ))}
        {candidate.pushedAt ? <span className="ml-auto shrink-0">最近更新 {relativeDays(candidate.pushedAt, props.now)}</span> : null}
      </div>

      <div className="mt-auto flex flex-wrap items-center gap-2 border-t border-muted pt-2.5">
        {status === 'mine' ? (
          <span className="inline-flex items-center gap-1 text-[13px] text-on-surface-variant">
            <Icon name="check_circle" size={16} fill className="text-[hsl(148_55%_42%)]" />
            已在你的 Stars
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
            收进收集箱
          </Button>
        )}
        <Button variant="text" size="sm" className="ml-auto" onClick={props.onCopyEvaluate}>
          复制给 Codex 评估
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
