/**
 * 全局设置上下文 Provider
 * 全局设置状态共享
 */

import { createContext, useContext, ReactNode, useEffect } from 'react';
import { useSettings } from '@/hooks/use-settings';
import { computeRootFontSize, computeThemeOverrides, managedThemeVariableNames } from '@/lib/theme-vars';
import type { AppSettings } from '@/types-settings';

type SettingsContextValue = ReturnType<typeof useSettings>;

const SettingsContext = createContext<SettingsContextValue | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const settings = useSettings();

  useEffect(() => {
    const cleanup = applyThemeSettings(settings.settings);
    return cleanup;
  }, [
    settings.settings.theme.brandColor,
    settings.settings.theme.colorPreset,
    settings.settings.theme.fontSize,
    settings.settings.theme.mode,
  ]);

  return <SettingsContext.Provider value={settings}>{children}</SettingsContext.Provider>;
}

export function useAppSettings(): SettingsContextValue {
  const context = useContext(SettingsContext);
  if (!context) {
    throw new Error('useAppSettings 必须在 <SettingsProvider> 内部使用');
  }
  return context;
}

function applyThemeSettings(settings: AppSettings) {
  const root = document.documentElement;
  const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

  const applyColorMode = () => {
    const isDark = settings.theme.mode === 'dark' || (settings.theme.mode === 'system' && mediaQuery.matches);
    root.classList.toggle('dark', isDark);
    root.style.colorScheme = isDark ? 'dark' : 'light';
  };

  for (const name of managedThemeVariableNames()) {
    root.style.removeProperty(name);
  }
  root.style.removeProperty('font-size');

  const variables = computeThemeOverrides(settings.theme);
  for (const [name, value] of Object.entries(variables)) {
    root.style.setProperty(name, value);
  }
  const rootFontSize = computeRootFontSize(settings.theme);
  if (rootFontSize) {
    root.style.fontSize = rootFontSize;
  }

  applyColorMode();

  if (settings.theme.mode !== 'system') {
    return undefined;
  }

  mediaQuery.addEventListener('change', applyColorMode);
  return () => mediaQuery.removeEventListener('change', applyColorMode);
}
