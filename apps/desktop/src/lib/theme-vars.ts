import type { ThemeSettings } from '@/types-settings';

/** 旧默认蓝。已保存的设置里还是这个值时，也不覆盖样式表。 */
const LEGACY_BRAND_COLOR = '#3b82f6';
/** 新色板主色。空字符串、旧默认、新默认都不算自定义。 */
const PALETTE_BRAND_COLOR = '#2e6bf0';

const FONT_SCALE: Record<ThemeSettings['fontSize'], number> = {
  small: 0.92,
  medium: 1,
  large: 1.08,
};

/** 新色板的字号基准：字号 / 行高。中号不写出这些变量。 */
const FONT_BASELINES = [
  ['--text-body-md', 14, 20],
  ['--text-body-lg', 16, 24],
  ['--text-headline-md', 20, 28],
  ['--text-headline-lg', 28, 36],
  ['--text-label-sm', 12, 16],
] as const;

const COLOR_VARIABLES = [
  '--color-primary',
  '--gsat-brand-color',
  '--color-on-primary',
  '--gsat-brand-foreground',
  '--color-primary-foreground',
  '--color-primary-container',
  '--color-on-primary-container',
  '--color-ring',
  '--color-accent',
  '--color-accent-foreground',
  '--color-primary-fixed',
  '--color-on-primary-fixed-variant',
] as const;

const FONT_VARIABLES = FONT_BASELINES.flatMap(([name]) => [name, `${name}--line-height`]);

/** 这份代码可能写过的 CSS 变量。应用前要全部移除，不能只清当前结果里的键。 */
export function managedThemeVariableNames(): readonly string[] {
  return [...COLOR_VARIABLES, ...FONT_VARIABLES];
}

/**
 * 默认品牌色和中号字返回空对象，让样式表生效。
 * 根元素的 font-size 不在这里，见 computeRootFontSize。
 */
export function computeThemeOverrides(theme: ThemeSettings): Record<string, string> {
  const variables: Record<string, string> = {};
  const brandColor = theme.brandColor.trim();

  if (!isDefaultBrand(brandColor)) {
    const foreground = readableForeground(brandColor);
    const hover = darkenHex(brandColor, 0.85);
    variables['--color-primary'] = brandColor;
    variables['--gsat-brand-color'] = brandColor;
    variables['--color-on-primary'] = foreground;
    variables['--gsat-brand-foreground'] = foreground;
    variables['--color-primary-foreground'] = foreground;
    variables['--color-primary-container'] = hover;
    variables['--color-on-primary-container'] = foreground;
    variables['--color-ring'] = brandColor;
    variables['--color-accent'] = colorWithAlpha(brandColor, 0.12);
    variables['--color-accent-foreground'] = brandColor;
    variables['--color-primary-fixed'] = colorWithAlpha(brandColor, 0.16);
    variables['--color-on-primary-fixed-variant'] = brandColor;
  }

  const scale = FONT_SCALE[theme.fontSize] ?? 1;
  if (scale !== 1) {
    for (const [name, size, lineHeight] of FONT_BASELINES) {
      variables[name] = px(size * scale);
      variables[`${name}--line-height`] = px(lineHeight * scale);
    }
  }

  return variables;
}

/** 中号返回 null，调用方不要设置根字号。小号、大号仍按 16px 基准缩放。 */
export function computeRootFontSize(theme: ThemeSettings): string | null {
  const scale = FONT_SCALE[theme.fontSize] ?? 1;
  if (scale === 1) {
    return null;
  }
  return px(16 * scale);
}

function isDefaultBrand(brandColor: string): boolean {
  const normalized = brandColor.trim().toLowerCase();
  return normalized.length === 0 || normalized === LEGACY_BRAND_COLOR || normalized === PALETTE_BRAND_COLOR;
}

function darkenHex(color: string, factor: number): string {
  const rgb = hexToRgb(color);
  if (!rgb) {
    return color;
  }
  const darkened = rgb.map((channel) => clampChannel(Math.round(channel * factor)));
  const unchanged = darkened.every((channel, index) => channel === rgb[index]);
  const channels = unchanged
    ? rgb.map((channel) => (channel > 0 ? channel - 1 : 0))
    : darkened;
  return `#${channels.map((channel) => channel.toString(16).padStart(2, '0')).join('')}`;
}

function colorWithAlpha(color: string, alpha: number): string {
  const normalizedAlpha = Math.max(0, Math.min(alpha, 1));
  const rgb = hexToRgb(color);
  if (!rgb) {
    return color;
  }
  return `rgb(${rgb[0]} ${rgb[1]} ${rgb[2]} / ${normalizedAlpha})`;
}

function readableForeground(backgroundColor: string): string {
  const rgb = hexToRgb(backgroundColor);
  const light = hexToRgb('#ffffff');
  const dark = hexToRgb('#111827');
  if (!rgb || !light || !dark) {
    return '#ffffff';
  }
  const lightContrast = contrastRatio(rgb, light);
  const darkContrast = contrastRatio(rgb, dark);
  return lightContrast >= darkContrast ? '#ffffff' : '#111827';
}

function contrastRatio(first: RgbColor, second: RgbColor): number {
  const lighter = Math.max(relativeLuminance(first), relativeLuminance(second));
  const darker = Math.min(relativeLuminance(first), relativeLuminance(second));
  return (lighter + 0.05) / (darker + 0.05);
}

function relativeLuminance(color: RgbColor): number {
  const [red, green, blue] = color.map((channel) => {
    const normalized = channel / 255;
    return normalized <= 0.03928 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}

function hexToRgb(color: string): RgbColor | null {
  const normalized = color.trim();
  const shortMatch = normalized.match(/^#([0-9a-f]{3})$/i);
  if (shortMatch) {
    const expanded = shortMatch[1]
      .split('')
      .map((channel) => parseInt(`${channel}${channel}`, 16));
    return [expanded[0], expanded[1], expanded[2]];
  }

  const longMatch = normalized.match(/^#([0-9a-f]{6})$/i);
  if (!longMatch) {
    return null;
  }
  return [
    parseInt(longMatch[1].slice(0, 2), 16),
    parseInt(longMatch[1].slice(2, 4), 16),
    parseInt(longMatch[1].slice(4, 6), 16),
  ];
}

function clampChannel(channel: number): number {
  return Math.max(0, Math.min(255, channel));
}

function px(value: number): string {
  const rounded = Math.round(value * 100) / 100;
  return `${rounded}px`;
}

type RgbColor = [number, number, number];
