/**
 * 应用更新上下文 Provider
 * 统一管理 Tauri updater 检查、下载、安装和重启状态。
 */

import { createContext, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { relaunch } from '@tauri-apps/plugin-process';
import { check, type Update } from '@tauri-apps/plugin-updater';
import type { DownloadEvent } from '@tauri-apps/plugin-updater';
import { describeUpdaterError, IN_APP_UPDATES_ENABLED, UPDATE_RELAUNCH_FAILED, UPDATE_WINDOW_STILL_OPEN } from '@/lib/api';

export type AppUpdateStatus =
  | 'idle'
  | 'checking'
  | 'available'
  | 'not-available'
  | 'downloading'
  | 'installed'
  | 'relaunching'
  | 'error';

export type AppUpdateDownloadProgress = {
  downloadedBytes: number;
  totalBytes: number | null;
  percent: number;
};

export type AppUpdateState = {
  status: AppUpdateStatus;
  currentVersion: string;
  availableVersion: string | null;
  releaseDate: string | null;
  releaseBody: string | null;
  lastCheckedAt: string | null;
  errorMessage: string | null;
  downloadProgress: AppUpdateDownloadProgress | null;
};

type CheckForUpdateOptions = {
  silent?: boolean;
};

export type AppUpdateContextValue = AppUpdateState & {
  checkForUpdate: (options?: CheckForUpdateOptions) => Promise<void>;
  installUpdate: () => Promise<void>;
  relaunchApp: () => Promise<void>;
};

const DEFAULT_UPDATE_STATE: AppUpdateState = {
  status: 'idle',
  currentVersion: '',
  availableVersion: null,
  releaseDate: null,
  releaseBody: null,
  lastCheckedAt: null,
  errorMessage: null,
  downloadProgress: null,
};

const AppUpdateContext = createContext<AppUpdateContextValue | null>(null);

export function AppUpdateProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<AppUpdateState>(DEFAULT_UPDATE_STATE);
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const pendingUpdateRef = useRef<Update | null>(null);
  const checkInFlightRef = useRef<Promise<void> | null>(null);
  const installInFlightRef = useRef(false);
  const relaunchInFlightRef = useRef(false);

  useEffect(() => {
    let cancelled = false;
    getVersion()
      .then((currentVersion) => {
        if (!cancelled) {
          setState((current) => ({ ...current, currentVersion }));
        }
      })
      .catch(() => undefined);

    return () => {
      cancelled = true;
    };
  }, []);

  async function checkForUpdate({ silent = false }: CheckForUpdateOptions = {}) {
    if (!IN_APP_UPDATES_ENABLED) {
      return;
    }

    if (installInFlightRef.current || relaunchInFlightRef.current) {
      return;
    }

    if (checkInFlightRef.current) {
      if (!silent) {
        setState((current) => ({ ...current, status: 'checking', errorMessage: null }));
      }
      await checkInFlightRef.current;
      return;
    }

    const checkTask = runUpdateCheck(silent);
    checkInFlightRef.current = checkTask;
    try {
      await checkTask;
    } finally {
      checkInFlightRef.current = null;
    }
  }

  async function installUpdate() {
    if (installInFlightRef.current || relaunchInFlightRef.current) {
      return;
    }

    const updateToInstall = pendingUpdateRef.current ?? pendingUpdate;
    if (!updateToInstall) {
      setState((current) => ({
        ...current,
        status: 'error',
        errorMessage: '当前没有可安装的更新，请先检查新版本。',
      }));
      return;
    }

    installInFlightRef.current = true;
    let downloadedBytes = 0;
    let totalBytes: number | null = null;
    setState((current) => ({
      ...current,
      status: 'downloading',
      errorMessage: null,
      downloadProgress: { downloadedBytes: 0, totalBytes: null, percent: 0 },
    }));

    try {
      await updateToInstall.downloadAndInstall((event: DownloadEvent) => {
        if (event.event === 'Started') {
          downloadedBytes = 0;
          totalBytes = typeof event.data.contentLength === 'number' ? event.data.contentLength : null;
        } else if (event.event === 'Progress') {
          downloadedBytes += event.data.chunkLength;
        } else if (event.event === 'Finished') {
          downloadedBytes = totalBytes ?? downloadedBytes;
        }

        setState((current) => ({
          ...current,
          downloadProgress: buildDownloadProgress(downloadedBytes, totalBytes),
        }));
      });

      setPendingUpdateValue(null);
      setState((current) => ({
        ...current,
        status: 'installed',
        errorMessage: null,
        downloadProgress: buildDownloadProgress(downloadedBytes, totalBytes),
      }));
      await requestRelaunch('automatic');
    } catch (error) {
      setState((current) => ({
        ...current,
        status: 'error',
        errorMessage: describeUpdaterError(error, 'install'),
      }));
    } finally {
      installInFlightRef.current = false;
    }
  }

  async function relaunchApp() {
    await requestRelaunch('manual');
  }

  const contextValue = useMemo<AppUpdateContextValue>(() => ({
    ...state,
    checkForUpdate,
    installUpdate,
    relaunchApp,
  }), [state, checkForUpdate, installUpdate, relaunchApp]);

  async function runUpdateCheck(silent: boolean) {
    setState((current) => ({
      ...current,
      status: silent ? current.status : 'checking',
      errorMessage: null,
    }));

    try {
      const update = await check();
      const checkedAt = new Date().toISOString();
      if (!update) {
        setPendingUpdateValue(null);
        setState((current) => ({
          ...current,
          status: 'not-available',
          availableVersion: null,
          releaseDate: null,
          releaseBody: null,
          lastCheckedAt: checkedAt,
          errorMessage: null,
          downloadProgress: null,
        }));
        return;
      }

      setPendingUpdateValue(update);
      setState((current) => ({
        ...current,
        status: 'available',
        currentVersion: update.currentVersion || current.currentVersion,
        availableVersion: update.version,
        releaseDate: update.date ?? null,
        releaseBody: update.body ?? null,
        lastCheckedAt: checkedAt,
        errorMessage: null,
        downloadProgress: null,
      }));
    } catch (error) {
      const checkedAt = new Date().toISOString();
      if (silent) {
        setState((current) => ({ ...current, status: current.status === 'checking' ? 'idle' : current.status }));
        return;
      }

      setPendingUpdateValue(null);
      setState((current) => ({
        ...current,
        status: 'error',
        lastCheckedAt: checkedAt,
        errorMessage: describeUpdaterError(error, 'check'),
      }));
    }
  }

  function setPendingUpdateValue(update: Update | null) {
    pendingUpdateRef.current = update;
    setPendingUpdate(update);
  }

  async function requestRelaunch(mode: 'automatic' | 'manual') {
    if (relaunchInFlightRef.current) {
      return;
    }

    relaunchInFlightRef.current = true;
    setState((current) => ({
      ...current,
      status: 'relaunching',
      errorMessage: null,
    }));

    try {
      await delay(mode === 'automatic' ? 800 : 150);
      await relaunch();
      await delay(1600);
      setState((current) => ({
        ...current,
        status: 'installed',
        errorMessage: UPDATE_WINDOW_STILL_OPEN,
      }));
    } catch {
      // 重启失败和「更新失败」是两回事：更新已经装好了，下一步是重启，不是重试更新
      setState((current) => ({
        ...current,
        status: 'installed',
        errorMessage: UPDATE_RELAUNCH_FAILED,
      }));
    } finally {
      relaunchInFlightRef.current = false;
    }
  }

  return <AppUpdateContext.Provider value={contextValue}>{children}</AppUpdateContext.Provider>;
}

export function useAppUpdate(): AppUpdateContextValue {
  const context = useContext(AppUpdateContext);
  if (!context) {
    throw new Error('useAppUpdate 必须在 <AppUpdateProvider> 内部使用');
  }
  return context;
}

function buildDownloadProgress(downloadedBytes: number, totalBytes: number | null): AppUpdateDownloadProgress {
  const normalizedDownloadedBytes = Math.max(0, downloadedBytes);
  const percent = totalBytes && totalBytes > 0
    ? Math.min(100, Math.round((normalizedDownloadedBytes / totalBytes) * 100))
    : 0;

  return {
    downloadedBytes: normalizedDownloadedBytes,
    totalBytes,
    percent,
  };
}

function delay(ms: number) {
  return new Promise<void>((resolve) => {
    window.setTimeout(resolve, ms);
  });
}
