import { useEffect } from 'react';
import type { ScenarioSummary, SolveHit } from '@/lib/contracts';
import { confirmLibraryCard, copyItemForCodex, describeInvokeError, setLibraryReading } from '@/lib/api';
import { readingGroupLabel, codexFallbackText } from '@/lib/item-format';
import { COPY_FOR_CODEX_TOAST, copyPromptToast } from '@/lib/solve-view';
import { useCopy } from '@/hooks/use-copy';
import { useLibraryDetail } from '@/hooks/use-library-detail';
import { useRegenerate } from '@/hooks/use-regenerate';
import { Button, useToast } from '@/components/kit';
import { ItemDetail } from '@/components/library';
import { Icon } from '@/components/ui/icon';
import { cn } from '@/lib/utils';

/**
 * 找方案里点一个结果：在右边滑出这个项目的详情（它是什么、怎么装、截图、README），不用离开这一页；
 * 看完直接在抽屉里决定放不放进交接单。Esc 或点遮罩关闭。
 */
export function HitDrawer(props: {
  hit: SolveHit;
  /** 这次想做的事（搜索的那句话），复制给 Codex 时带上 */
  problem?: string;
  scenarios: ScenarioSummary[];
  picked: boolean;
  isMain: boolean;
  onTogglePick: () => void;
  onMakeMain: () => void;
  onClose: () => void;
  onOpenToolbox?: () => void;
}) {
  const toast = useToast();
  const copy = useCopy();
  const regenerate = useRegenerate(props.onOpenToolbox);
  const { detail, loading, error, retry, paintReading } = useLibraryDetail(props.hit.id);

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key === 'Escape') props.onClose();
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [props]);

  async function changeReading(group: Parameters<typeof setLibraryReading>[1]) {
    try {
      await setLibraryReading([props.hit.id], group);
      paintReading(group);
      toast.show({ title: `已标为「${readingGroupLabel(group)}」`, sub: props.hit.fullName });
    } catch (reason) {
      toast.show({ title: describeInvokeError(reason) });
    }
  }

  async function copyForCodex() {
    let text = '';
    try {
      text = await copyItemForCodex(props.hit.id, props.problem);
    } catch (reason) {
      const message = describeInvokeError(reason);
      if (message.includes('这个功能还没接上') && detail) {
        text = codexFallbackText(detail);
      } else {
        toast.show({ title: message });
        return;
      }
    }
    await copy(text, COPY_FOR_CODEX_TOAST, props.hit.fullName);
  }

  return (
    <div className="fixed inset-0 z-40 flex justify-end bg-black/25" onClick={props.onClose} role="presentation">
      <aside
        className="flex h-full w-[min(560px,100vw)] flex-col bg-card shadow-lg"
        role="dialog"
        aria-label={`${props.hit.fullName} 的详情`}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex shrink-0 items-center gap-2 border-b border-border px-4 py-2.5">
          <Button variant="text" onClick={props.onClose} aria-label="关闭详情">
            <Icon name="close" size={18} />
            关闭
          </Button>
          <span className="min-w-0 flex-1 truncate text-sm font-medium text-foreground">{props.hit.fullName}</span>
          {props.picked && !props.isMain ? <Button size="sm" onClick={props.onMakeMain}>设为主参考</Button> : null}
          {props.picked && props.isMain ? <span className="text-xs font-medium text-accent-foreground">主参考</span> : null}
          <Button variant={props.picked ? 'default' : 'primary'} className={cn(props.picked && 'text-on-surface-variant')} onClick={props.onTogglePick}>
            <Icon name={props.picked ? 'remove' : 'add'} size={16} />
            {props.picked ? '从交接单移除' : '加入交接单'}
          </Button>
        </div>
        <div className="flex min-h-0 flex-1 flex-col">
          <ItemDetail
            detail={detail}
            loading={loading}
            error={error}
            scenarios={props.scenarios}
            onRetry={retry}
            onReadingChange={(group) => { void changeReading(group); }}
            onConfirm={() => {
              void confirmLibraryCard(props.hit.id)
                .then(() => toast.show({ title: '已确认' }))
                .catch((reason: unknown) => toast.show({ title: describeInvokeError(reason) }));
            }}
            onRegenerate={() => { void regenerate(props.hit.id); }}
            onOpenToolbox={props.onOpenToolbox}
            onCopyCodex={() => { void copyForCodex(); }}
            onCopyHow={(text) => { void copy(text, '已复制'); }}
            onCopyPrompt={(text) => { void copy(text, copyPromptToast(text)); }}
          />
        </div>
      </aside>
    </div>
  );
}
