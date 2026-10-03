import { describe, expect, it } from 'vitest';
import { DEFAULT_SETTINGS, type ThemeSettings } from '@/types-settings';
import { computeRootFontSize, computeThemeOverrides } from './theme-vars';

function theme(partial: Partial<ThemeSettings>): ThemeSettings {
  return { ...DEFAULT_SETTINGS.theme, ...partial };
}

function channelSum(color: string): number {
  const match = color.trim().match(/^#([0-9a-f]{6})$/i);
  if (!match) {
    throw new Error(`不是 6 位色值：${color}`);
  }
  return (
    parseInt(match[1].slice(0, 2), 16) +
    parseInt(match[1].slice(2, 4), 16) +
    parseInt(match[1].slice(4, 6), 16)
  );
}

describe('主题变量', () => {
  it('默认设置不写任何变量', () => {
    expect(computeThemeOverrides(DEFAULT_SETTINGS.theme)).toEqual({});
    expect(computeRootFontSize(DEFAULT_SETTINGS.theme)).toBeNull();
  });

  it('新旧默认品牌色不产生颜色变量', () => {
    for (const brandColor of ['#3b82f6', '#3B82F6', '#2E6BF0', '#2e6bf0']) {
      const variables = computeThemeOverrides(theme({ brandColor, fontSize: 'medium' }));
      const colorNames = Object.keys(variables).filter(
        (name) => name.startsWith('--color-') || name.startsWith('--gsat-'),
      );
      expect(colorNames).toEqual([]);
    }
  });

  it('自定义品牌色写出变量且悬停色更深', () => {
    const brand = '#8b5cf6';
    const variables = computeThemeOverrides(theme({ brandColor: brand, fontSize: 'medium' }));
    expect(variables['--color-primary']).toBe(brand);
    expect(variables['--gsat-brand-color']).toBe(brand);
    expect(variables['--color-ring']).toBe(brand);
    expect(variables['--color-accent-foreground']).toBe(brand);
    expect(variables['--color-on-primary-fixed-variant']).toBe(brand);
    expect(variables['--color-on-primary']).toBe('#ffffff');
    expect(variables['--gsat-brand-foreground']).toBe('#ffffff');
    expect(variables['--color-primary-foreground']).toBe('#ffffff');
    expect(variables['--color-on-primary-container']).toBe('#ffffff');
    expect(variables['--color-accent']).toBe('rgb(139 92 246 / 0.12)');
    expect(variables['--color-primary-fixed']).toBe('rgb(139 92 246 / 0.16)');
    expect(variables['--color-primary-container']).toBe('#764ed1');
    expect(variables['--color-primary-container']).not.toBe(brand);
    expect(channelSum(variables['--color-primary-container'])).toBeLessThan(channelSum(brand));
  });

  it('中号字不产生字号变量', () => {
    const variables = computeThemeOverrides(theme({ brandColor: '#8b5cf6', fontSize: 'medium' }));
    const fontNames = Object.keys(variables).filter((name) => name.startsWith('--text-'));
    expect(fontNames).toEqual([]);
    expect(computeRootFontSize(theme({ fontSize: 'medium' }))).toBeNull();
  });

  it('小号和大号按新基准缩放', () => {
    const small = computeThemeOverrides(theme({ fontSize: 'small' }));
    expect(small['--text-body-md']).toBe('12.88px');
    expect(small['--text-body-md--line-height']).toBe('18.4px');
    expect(small['--text-body-lg']).toBe('14.72px');
    expect(small['--text-body-lg--line-height']).toBe('22.08px');
    expect(small['--text-headline-md']).toBe('18.4px');
    expect(small['--text-headline-md--line-height']).toBe('25.76px');
    expect(small['--text-headline-lg']).toBe('25.76px');
    expect(small['--text-headline-lg--line-height']).toBe('33.12px');
    expect(small['--text-label-sm']).toBe('11.04px');
    expect(small['--text-label-sm--line-height']).toBe('14.72px');
    expect(computeRootFontSize(theme({ fontSize: 'small' }))).toBe('14.72px');
    expect(Object.keys(small).some((name) => name.startsWith('--color-'))).toBe(false);

    const large = computeThemeOverrides(theme({ fontSize: 'large' }));
    expect(large['--text-body-md']).toBe('15.12px');
    expect(large['--text-body-md--line-height']).toBe('21.6px');
    expect(large['--text-body-lg']).toBe('17.28px');
    expect(large['--text-body-lg--line-height']).toBe('25.92px');
    expect(large['--text-headline-md']).toBe('21.6px');
    expect(large['--text-headline-md--line-height']).toBe('30.24px');
    expect(large['--text-headline-lg']).toBe('30.24px');
    expect(large['--text-headline-lg--line-height']).toBe('38.88px');
    expect(large['--text-label-sm']).toBe('12.96px');
    expect(large['--text-label-sm--line-height']).toBe('17.28px');
    expect(computeRootFontSize(theme({ fontSize: 'large' }))).toBe('17.28px');
  });

  it('空品牌色按默认处理', () => {
    expect(computeThemeOverrides(theme({ brandColor: '', fontSize: 'medium' }))).toEqual({});
    expect(computeRootFontSize(theme({ brandColor: '', fontSize: 'medium' }))).toBeNull();
  });

  it('返回的变量名都以 -- 开头', () => {
    const variables = computeThemeOverrides(theme({ brandColor: '#8b5cf6', fontSize: 'large' }));
    const names = Object.keys(variables);
    expect(names.length).toBeGreaterThan(0);
    expect(names.every((name) => name.startsWith('--'))).toBe(true);
  });
});
