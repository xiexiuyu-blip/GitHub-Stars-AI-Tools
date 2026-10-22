import { MATCH_STAR_HINTS, MATCH_STAR_MAX, matchReasonsLine, normalizeMatchStars } from '@/lib/match-hint';

/** 从矮到高，对应第 1 到第 4 根。 */
const BAR_HEIGHTS = ['h-[6px]', 'h-[8px]', 'h-[10px]', 'h-[12px]'] as const;

/** 搜索命中：左边 1–4 档竖条，右边「命中：…」。两样都没有就不画。 */
export function MatchHint(props: { reasons?: readonly string[] | null; stars?: number | null }) {
  const stars = normalizeMatchStars(props.stars);
  const line = matchReasonsLine(props.reasons);
  if (stars == null && line == null) return null;
  return (
    <div
      className="mt-1.5 flex items-center gap-2 text-xs text-on-surface-variant"
      title={stars != null ? MATCH_STAR_HINTS[stars] : undefined}
    >
      {stars != null ? (
        <span role="img" aria-label={`匹配度 ${stars} / ${MATCH_STAR_MAX}`} className="inline-flex items-end gap-[2px]">
          {BAR_HEIGHTS.map((height, index) => (
            <i
              key={height}
              className={`w-[3px] rounded-[1px] ${height} ${index < stars ? 'bg-primary' : 'bg-border'}`}
            />
          ))}
        </span>
      ) : null}
      {line != null ? <span className="min-w-0 truncate">{line}</span> : null}
    </div>
  );
}
