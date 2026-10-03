import { useEffect, useState } from 'react';
import { cn } from '@/lib/utils';
import { avatarUrl, hueOf, ownerOf } from '@/lib/repo-assets';
import { splitRepoName } from '@/lib/item-format';

const SIZE_PX = { sm: 32, md: 40, lg: 56 } as const;

/**
 * 仓库所有者的头像（GitHub 的图）。图没加载出来或离线时，退回一个按名字上色的首字母方块，
 * 同一个所有者永远同一种颜色，列表里一眼能分出「这几个是同一个人的」。
 */
export function RepoAvatar(props: { name: string; size?: keyof typeof SIZE_PX; className?: string }) {
  const px = SIZE_PX[props.size ?? 'sm'];
  const url = avatarUrl(props.name, px * 2);
  const [failed, setFailed] = useState(false);
  const [loaded, setLoaded] = useState(false);
  useEffect(() => {
    setFailed(false);
    setLoaded(false);
  }, [url]);
  const hue = hueOf(ownerOf(props.name) || props.name);
  return (
    <span
      className={cn('relative inline-grid shrink-0 place-items-center overflow-hidden rounded-[10px] font-semibold select-none', props.className)}
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
      {url && !failed ? (
        <img
          src={url}
          alt=""
          loading="lazy"
          decoding="async"
          referrerPolicy="no-referrer"
          onLoad={() => setLoaded(true)}
          onError={() => setFailed(true)}
          className={cn('absolute inset-0 size-full bg-card object-cover transition-opacity duration-150', loaded ? 'opacity-100' : 'opacity-0')}
        />
      ) : null}
    </span>
  );
}
