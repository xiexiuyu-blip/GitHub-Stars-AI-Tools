type BrandIconProps = {
  className?: string;
  markClassName?: string;
  title?: string;
  /** brand：蓝底浅色标记，用在顶栏。默认保持原来的浅底蓝标。 */
  tone?: 'default' | 'brand';
};

export function BrandIcon({ className = '', markClassName = '', title, tone = 'default' }: BrandIconProps) {
  const shellTone = tone === 'brand'
    ? 'border border-transparent bg-primary'
    : 'border border-card-border bg-surface-container-lowest';
  const markTone = tone === 'brand' ? 'bg-on-primary' : 'bg-primary';

  return (
    <span
      className={`app-brand-icon inline-flex shrink-0 items-center justify-center ${shellTone} ${className}`}
      title={title}
      aria-hidden={title ? undefined : true}
      role={title ? 'img' : undefined}
    >
      <span
        className={`block h-[84%] w-[84%] ${markTone} ${markClassName}`}
        style={{
          WebkitMask: 'url("/brand-mark.svg") center / contain no-repeat',
          mask: 'url("/brand-mark.svg") center / contain no-repeat',
        }}
      />
    </span>
  );
}
