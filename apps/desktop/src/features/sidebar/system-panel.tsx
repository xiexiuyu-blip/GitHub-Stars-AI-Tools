import { Database } from 'lucide-react';
import type { BackendStatus, RepositoryStats } from '@/types';

type SystemPanelProps = {
  status: BackendStatus | null;
  stats: RepositoryStats;
};

export function SystemPanel(props: SystemPanelProps) {
  return (
    <section className="rail-section">
      <div className="rail-title">
        <Database className="size-4" />
        <strong>资料库状态</strong>
      </div>
      <div className="grid grid-cols-2 gap-2.5">
        <Metric label="工具" value={props.stats.total.toLocaleString()} />
        <Metric label="项目说明" value={props.stats.withReadme.toLocaleString()} />
        <Metric label="语言" value={props.stats.languages.toLocaleString()} />
        <Metric label="GitHub 话题" value={props.stats.topics.toLocaleString()} />
      </div>
      <div className="grid gap-2 rounded-lg bg-muted/30 p-3 text-xs leading-relaxed text-muted-foreground">
        <span className="flex items-center gap-2">
          <span className="size-1.5 rounded-full bg-success" />
          {props.status?.backend ?? '本地服务正在启动'}
        </span>
        <span className="flex items-center gap-2">
          <span className="size-1.5 rounded-full bg-warning" />
          {props.status?.worker ?? '后台任务正在准备'}
        </span>
        <span className="flex items-center gap-2">
          <span className="size-1.5 rounded-full bg-muted-foreground" />
          {props.status?.provider ?? 'AI 还没设置'}
        </span>
      </div>
    </section>
  );
}

function Metric(props: { label: string; value: string }) {
  return (
    <span className="rounded-lg border bg-card p-3 shadow-sm transition-all hover:shadow-md">
      <strong className="block text-base font-semibold tabular-nums">{props.value}</strong>
      <small className="text-xs text-muted-foreground">{props.label}</small>
    </span>
  );
}
