import { ButtonHTMLAttributes } from 'react';
import { cn } from '@/lib/utils';
import { Spinner } from './spinner';

/**
 * 按钮。
 * variant="primary" 是蓝色实底主按钮：一个页面里只能有一个。
 * 其余操作用 default（白底细边框）或 text（文字链接）。
 */
type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: 'primary' | 'default' | 'text';
  size?: 'md' | 'sm';
  loading?: boolean;
};

export function Button(props: ButtonProps) {
  const {
    variant = 'default',
    size = 'md',
    loading = false,
    disabled,
    className,
    children,
    type = 'button',
    ...rest
  } = props;

  return (
    <button
      type={type}
      disabled={disabled || loading}
      className={cn(
        'inline-flex shrink-0 items-center justify-center gap-1.5 rounded-md text-sm whitespace-nowrap transition-colors',
        'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-primary',
        'disabled:cursor-not-allowed disabled:opacity-45',
        size === 'sm' ? 'h-7 px-2.5' : 'h-8 px-[14px]',
        variant === 'primary' && 'border border-primary bg-primary text-on-primary hover:border-primary-container hover:bg-primary-container',
        variant === 'default' && 'border border-input bg-card text-foreground hover:bg-surface-container-low',
        variant === 'text' && 'h-7 border border-transparent bg-transparent px-1 text-accent-foreground hover:bg-transparent hover:underline hover:underline-offset-[3px]',
        className,
      )}
      {...rest}
    >
      {loading ? <Spinner /> : null}
      {children}
    </button>
  );
}
