import { useState } from 'react';
import { cn } from '@/lib/utils';
import { avatarUrl, hueOf, ownerOf } from '@/lib/repo-assets';
import { splitRepoName } from '@/lib/item-format';

const SIZE_PX = { sm: 32, md: 40, lg: 56 } as const;

type ImageState = { url: string | null; status: 'loading' | 'loaded' | 'failed' };

/**
 * 仓库所有者的头像（GitHub 的图）。图没加载出来或离线时，退回一个按名字上色的首字母方块，
 * 同一个所有者永远同一种颜色，列表里一眼能分出「这几个是同一个人的」。
 *
 * 加载状态记的是「哪个地址加载好了」，不是一个会被副作用重置的布尔值：
 * 图片在缓存里时 load 事件可能比 React 的副作用还早，之前用 useEffect 重置状态，会把刚加载好的状态又清掉，图就一直透明。
 */
export function RepoAvatar(props: { name: string; size?: keyof typeof SIZE_PX; className?: string }) {
  const px = SIZE_PX[props.size ?? 'sm'];
  const url = avatarUrl(props.name, px * 2);
  const [state, setState] = useState<ImageState>({ url: null, status: 'loading' });
  const status = state.url === url ? state.status : 'loading';
  const hue = hueOf(ownerOf(props.name) || props.name);
  return (
    <span
      className={cn('relative inline-grid shrink-0 place-items-center overflow-hidden rounded-lg font-semibold select-none', props.className)}
      style={{
        width: px,
        height: px,
        background: `hsl(${hue} 62% 90%)`,
        color: `hsl(${hue} 45% 30%)`,
        fontSize: Math.round(px * 0.42),
      }}
      aria-hidden="true"
    >
      {splitRepoName(props.name).initial}
      {url && status !== 'failed' ? (
        <img
          key={url}
          ref={(element) => {
            // 已经在缓存里、挂上来时就加载完了：不用等 load 事件
            if (element && element.complete && element.naturalWidth > 0 && status === 'loading') {
              setState({ url, status: 'loaded' });
            }
          }}
          src={url}
          alt=""
          loading="lazy"
          decoding="async"
          referrerPolicy="no-referrer"
          onLoad={() => setState({ url, status: 'loaded' })}
          onError={() => setState({ url, status: 'failed' })}
          className={cn('absolute inset-0 size-full bg-card object-cover transition-opacity duration-150', status === 'loaded' ? 'opacity-100' : 'opacity-0')}
        />
      ) : null}
    </span>
  );
}
