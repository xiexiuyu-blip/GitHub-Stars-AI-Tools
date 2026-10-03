import { useState } from 'react';
import { cn } from '@/lib/utils';
import { coverGradient, hueOf } from '@/lib/repo-assets';
import { splitRepoName } from '@/lib/item-format';

type ImageState = { src: string | null; status: 'loading' | 'loaded' | 'failed'; ratio: number };

/**
 * 封面：优先用 README 里的第一张截图；没有、没加载出来、离线时是按名字上色的渐变底（可选加一个大字母），
 * 所以每个仓库都有「脸」，不会出现一块空白。
 *
 * fit="cover"（默认）：填满并对齐顶部，适合小缩略图；很宽的横幅 / 很高的长图自动改成完整显示。
 * fit="contain"：完整显示整张图，四周用这张图自己模糊放大的样子垫底，不裁掉、也不留白边。
 *
 * 加载状态记的是「哪个地址加载好了」而不是会被副作用重置的布尔值（缓存里的图 load 事件可能比副作用早，会被清掉，图就一直透明）。
 */
export function CoverImage(props: {
  src?: string | null;
  seed: string;
  alt?: string;
  className?: string;
  letter?: boolean;
  fit?: 'cover' | 'contain';
  onFail?: () => void;
}) {
  const src = props.src ?? null;
  const [state, setState] = useState<ImageState>({ src: null, status: 'loading', ratio: 1.6 });
  const current = state.src === src ? state : { src, status: 'loading' as const, ratio: 1.6 };
  const showImage = Boolean(src) && current.status !== 'failed';
  const loaded = showImage && current.status === 'loaded';
  const odd = current.ratio > 2.3 || current.ratio < 0.75;
  const contain = props.fit === 'contain' || odd;
  const hue = hueOf(props.seed);

  function settle(element: HTMLImageElement) {
    if (element.naturalHeight <= 0) return;
    setState({ src, status: 'loaded', ratio: element.naturalWidth / element.naturalHeight });
  }

  return (
    <div className={cn('relative overflow-hidden', props.className)} style={{ background: coverGradient(props.seed) }}>
      {props.letter !== false && !loaded ? (
        <span
          className="absolute inset-0 grid place-items-center text-[56px] leading-none font-bold select-none"
          style={{ color: `hsl(${hue} 45% 38% / 0.16)` }}
          aria-hidden="true"
        >
          {splitRepoName(props.seed).initial}
        </span>
      ) : null}
      {loaded && contain && src ? (
        <span
          className="absolute -inset-6 bg-cover bg-center opacity-60 blur-xl saturate-150"
          style={{ backgroundImage: `url("${src.replace(/"/g, '%22')}")` }}
          aria-hidden="true"
        />
      ) : null}
      {showImage ? (
        <img
          key={src}
          ref={(element) => {
            if (element && element.complete && current.status === 'loading') settle(element);
          }}
          src={src ?? undefined}
          alt={props.alt ?? ''}
          loading="lazy"
          decoding="async"
          referrerPolicy="no-referrer"
          onLoad={(event) => settle(event.currentTarget)}
          onError={() => {
            setState({ src, status: 'failed', ratio: 1.6 });
            props.onFail?.();
          }}
          className={cn(
            'absolute inset-0 size-full transition-opacity duration-200',
            contain ? 'object-contain' : 'object-cover object-top',
            loaded ? 'opacity-100' : 'opacity-0',
          )}
        />
      ) : null}
    </div>
  );
}
