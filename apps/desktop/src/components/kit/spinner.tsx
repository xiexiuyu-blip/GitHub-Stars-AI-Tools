import { cn } from '@/lib/utils';

export function Spinner(props: { className?: string }) {
  return <span className={cn('kit-spinner', props.className)} aria-hidden="true" />;
}
