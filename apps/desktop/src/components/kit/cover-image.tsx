import { useEffect, useState } from 'react';
import { cn } from '@/lib/utils';
import { coverGradient, hueOf } from '@/lib/repo-assets';
import { splitRepoName } from '@/lib/item-format';

/**
 * 封面：优先用 README 里的第一张截图；没有、没加载出来、离线时是按名字上色的渐变底加一个大字母，
 * 所以每个仓库都有「脸」，不会出现一块空白。
 */
export function CoverImage(props: { src?: string | null; seed: string; alt?: string; className?: string; letter?: boolean; onFail?: () => void }) {
  const [failed, setFailed] = useState(false);
  const [loaded, setLoaded] = useState(false);
  // 很宽的横幅（banner）或很高的长图：用 contain 完整显示，不裁掉；普通截图用 cover 填满并对齐顶部
  const [ratio, setRatio] = useState(1.6);
  useEffect(() => {
    setFailed(false);
    setLoaded(false);
  }, [props.src]);
  const odd = ratio > 2.3 || ratio < 0.75;
  const hue = hueOf(props.seed);
  const showImage = Boolean(props.src) && !failed;
  return (
    <div className={cn('relative overflow-hidden', props.className)} style={{ background: coverGradient(props.seed) }}>
      {props.letter !== false && !(showImage && loaded) ? (
        <span
          className="absolute inset-0 grid place-items-center text-[56px] leading-none font-bold select-none"
          style={{ color: `hsl(${hue} 45% 38% / 0.16)` }}
          aria-hidden="true"
        >
          {splitRepoName(props.seed).initial}
        </span>
      ) : null}
      {showImage ? (
        <img
          src={props.src ?? undefined}
          alt={props.alt ?? ''}
          loading="lazy"
          decoding="async"
          referrerPolicy="no-referrer"
          onLoad={(event) => {
            const image = event.currentTarget;
            if (image.naturalHeight > 0) setRatio(image.naturalWidth / image.naturalHeight);
            setLoaded(true);
          }}
          onError={() => {
            setFailed(true);
            props.onFail?.();
          }}
          className={cn(
            'absolute inset-0 size-full transition-opacity duration-200',
            odd ? 'bg-card object-contain p-1.5' : 'object-cover object-top',
            loaded ? 'opacity-100' : 'opacity-0',
          )}
        />
      ) : null}
    </div>
  );
}
