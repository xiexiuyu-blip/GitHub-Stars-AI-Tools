import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { FormEvent, useEffect, useMemo, useRef, useState } from 'react';
import { getAiConfigMessage, toBackendAiRequestConfig } from '@/lib/ai-config';
import { formatSyncComplete } from '@/lib/automation-view';
import { getTodayView, plainTechnicalError } from '@/lib/api';
import { invalidateRepoReadmesForProgress } from '@/hooks/use-repo-readme';
import { emptyRepositoryFilters, getRepositoryStats } from '@/lib/repository';
import { optionalRequestText } from '@/lib/format';
import type {
  AISettings,
  AiStreamEvent,
  AiTagNetworkSummary,
  BatchAiDocumentSummary,
  BackendStatus,
  GistAnnotationExportSummary,
  GistAnnotationImportSummary,
  GistRepositoryLibraryExportSummary,
  GistRepositoryLibraryImportSummary,
  GitHubAuthState,
  GitHubUser,
  GithubRecommendationPage,
  GithubRecommendationResponse,
  ReadmeFetchSummary,
  ReadingStatus,
  RepositoryAnnotationView,
  RepositoryDetailView,
  RepositoryFilterCounts,
  RepositoryFilters,
  RepositoryListItem,
  RepositoryListPage,
  StarSyncSummary,
  TagItem,
  TaskProgressEvent,
} from '@/types';

const initialAuthState: GitHubAuthState = {
  hasToken: false,
  user: null,
};

const REPOSITORY_PAGE_SIZE = 5000;

const emptyRepositoryPage: RepositoryListPage = {
  items: [],
  totalCount: 0,
  limit: REPOSITORY_PAGE_SIZE,
  offset: 0,
};

const emptyRepositoryFilterCounts: RepositoryFilterCounts = {
  totalCount: 0,
  languageCounts: {},
  tagCounts: {},
};

class ReadmePostProcessWarning extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ReadmePostProcessWarning';
  }
}

type RepositoryAiStreamState = {
  requestId: string;
  repositoryId: string;
  stage: string;
  status: string;
  message: string | null;
  text: string;
  startedAt: string;
  updatedAt: string;
};

export function useStarsWorkspace() {
  const repositoryLoadSequence = useRef(0);
  const annotationLoadSequence = useRef(0);
  const selectedRepositoryIdRef = useRef<string | null>(null);
  const [status, setStatus] = useState<BackendStatus | null>(null);
  const [authState, setAuthState] = useState<GitHubAuthState>(initialAuthState);
  const [token, setToken] = useState('');
  const [isLoadingAuth, setIsLoadingAuth] = useState(true);
  const [isSavingToken, setIsSavingToken] = useState(false);
  const [isClearingToken, setIsClearingToken] = useState(false);
  const [isClearingLocalData, setIsClearingLocalData] = useState(false);
  const [isSyncingStars, setIsSyncingStars] = useState(false);
  const [isFetchingReadmes, setIsFetchingReadmes] = useState(false);
  const [isExportingAnnotations, setIsExportingAnnotations] = useState(false);
  const [isImportingAnnotations, setIsImportingAnnotations] = useState(false);
  const [isExportingRepositoryLibrary, setIsExportingRepositoryLibrary] = useState(false);
  const [isImportingRepositoryLibrary, setIsImportingRepositoryLibrary] = useState(false);
  const [isLoadingRepositories, setIsLoadingRepositories] = useState(false);
  const [isLoadingAnnotation, setIsLoadingAnnotation] = useState(false);
  const [isLoadingRepositoryDetail, setIsLoadingRepositoryDetail] = useState(false);
  const [isSavingAnnotation, setIsSavingAnnotation] = useState(false);
  const [isSavingTag, setIsSavingTag] = useState(false);
  const [isFetchingRepositoryReadme, setIsFetchingRepositoryReadme] = useState(false);
  const [isGeneratingAiDocument, setIsGeneratingAiDocument] = useState(false);
  const [isBatchGeneratingAiDocuments, setIsBatchGeneratingAiDocuments] = useState(false);
  const [isGeneratingTagNetwork, setIsGeneratingTagNetwork] = useState(false);
  const [isFindingSimilarRepositories, setIsFindingSimilarRepositories] = useState(false);
  const [isStarringRecommendationCandidate, setIsStarringRecommendationCandidate] = useState(false);
  const [batchAiSummary, setBatchAiSummary] = useState<BatchAiDocumentSummary | null>(null);
  const [githubRecommendationResponse, setGithubRecommendationResponse] = useState<GithubRecommendationResponse | null>(null);
  const [githubRecommendationError, setGithubRecommendationError] = useState<string | null>(null);
  const [lastGithubRecommendationRepositoryIds, setLastGithubRecommendationRepositoryIds] = useState<string[]>([]);
  const [lastStarRecommendationCandidateFullName, setLastStarRecommendationCandidateFullName] = useState<string | null>(null);
  const [syncSummary, setSyncSummary] = useState<StarSyncSummary | null>(null);
  const [lastSyncedAt, setLastSyncedAt] = useState<string | null>(null);
  const [readmeSummary, setReadmeSummary] = useState<ReadmeFetchSummary | null>(null);
  const [repositoryPage, setRepositoryPage] = useState<RepositoryListPage | null>(null);
  const [repositoryLanguages, setRepositoryLanguages] = useState<string[]>([]);
  const [repositoryFilterCounts, setRepositoryFilterCounts] = useState<RepositoryFilterCounts>(emptyRepositoryFilterCounts);
  const [repositoryFilters, setRepositoryFilters] = useState<RepositoryFilters>(emptyRepositoryFilters);
  const [selectedRepositoryId, setSelectedRepositoryId] = useState<string | null>(null);
  const [tags, setTags] = useState<TagItem[]>([]);
  const [annotation, setAnnotation] = useState<RepositoryAnnotationView | null>(null);
  const [repositoryDetail, setRepositoryDetail] = useState<RepositoryDetailView | null>(null);
  const [noteDraft, setNoteDraft] = useState('');
  const [readingStatusDraft, setReadingStatusDraft] = useState<ReadingStatus>('unread');
  const [newTagName, setNewTagName] = useState('');
  const [newTagColor, setNewTagColor] = useState('#f5f5f5');
  const [gistIdDraft, setGistIdDraft] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [authMessage, setAuthMessage] = useState<string | null>(null);
  const [annotationMessage, setAnnotationMessage] = useState<string | null>(null);
  const [taskProgress, setTaskProgress] = useState<TaskProgressEvent | null>(null);
  const [repositoryAiStream, setRepositoryAiStream] = useState<RepositoryAiStreamState | null>(null);
  const [repositoryReadmeError, setRepositoryReadmeError] = useState<{ repositoryId: string; message: string } | null>(null);
  const [repositoryAiError, setRepositoryAiError] = useState<{ repositoryId: string; message: string } | null>(null);
  const [isRetryingAiSearch, setIsRetryingAiSearch] = useState(false);
  const lastAiSearchRetryRef = useRef<(() => Promise<void>) | null>(null);

  const selectedRepository = useMemo(
    () => repositoryPage?.items.find((repository) => repository.id === selectedRepositoryId) ?? null,
    [repositoryPage, selectedRepositoryId],
  );
  const repositoryStats = useMemo(() => getRepositoryStats(repositoryPage), [repositoryPage]);

  useEffect(() => {
    let unlistenProgress: (() => void) | null = null;
    let unlistenAiStream: (() => void) | null = null;
    void listen<TaskProgressEvent>('task-progress', (event) => {
      invalidateRepoReadmesForProgress(event.payload);
      setTaskProgress(event.payload);
    }).then((nextUnlisten) => {
      unlistenProgress = nextUnlisten;
    });
    void listen<AiStreamEvent>('ai-stream', (event) => {
      const payload = event.payload;
      if (payload.taskType !== 'repository-ai-document' || !payload.repositoryId) {
        return;
      }
      const repositoryId = payload.repositoryId;
      setRepositoryAiStream((current) => {
        if (current && current.requestId !== payload.requestId) {
          return current;
        }
        if (!current && selectedRepositoryIdRef.current !== repositoryId) {
          return current;
        }
        const startedAt = current?.startedAt ?? payload.createdAt;
        return {
          requestId: payload.requestId,
          repositoryId,
          stage: payload.stage,
          status: payload.status,
          message: payload.message ?? current?.message ?? null,
          text: payload.text ?? current?.text ?? '',
          startedAt,
          updatedAt: payload.createdAt,
        };
      });
    }).then((nextUnlisten) => {
      unlistenAiStream = nextUnlisten;
    });

    invoke<BackendStatus>('get_backend_status')
      .then(setStatus)
      .catch((reason: unknown) => setError(toErrorMessage(reason)));

    invoke<GitHubAuthState>('get_github_auth_state')
      .then(setAuthState)
      .catch((reason: unknown) => setError(toErrorMessage(reason)))
      .finally(() => setIsLoadingAuth(false));

    loadRepositories(emptyRepositoryFilters);
    loadRepositoryLanguages();

    return () => {
      unlistenProgress?.();
      unlistenAiStream?.();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    void getTodayView()
      .then((view) => {
        if (!cancelled) {
          setLastSyncedAt(view.syncedAt);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (taskProgress?.status !== 'succeeded') {
      return undefined;
    }

    const completedProgress = taskProgress;
    const timeoutId = window.setTimeout(() => {
      setTaskProgress((current) => current === completedProgress ? null : current);
    }, 6000);
    return () => window.clearTimeout(timeoutId);
  }, [taskProgress]);

  useEffect(() => {
    selectedRepositoryIdRef.current = selectedRepositoryId;
  }, [selectedRepositoryId]);

  useEffect(() => {
    if (!repositoryPage || repositoryPage.items.length === 0) {
      setSelectedRepositoryId(null);
      return;
    }

    const selectedStillExists = repositoryPage.items.some((repository) => repository.id === selectedRepositoryId);

    if (!selectedStillExists) {
      setSelectedRepositoryId(repositoryPage.items[0].id);
    }
  }, [repositoryPage, selectedRepositoryId]);

  useEffect(() => {
    if (!selectedRepository) {
      annotationLoadSequence.current += 1;
      setAnnotation(null);
      setRepositoryDetail(null);
      setRepositoryAiStream(null);
      setNoteDraft('');
      setReadingStatusDraft('unread');
      return;
    }

    loadAnnotationWorkspace(selectedRepository);
  }, [selectedRepository?.id, selectedRepository?.accountId]);

  useEffect(() => {
    void refreshRepositoryWorkspace();
  }, [authState.user?.id]);

  async function loadRepositories(nextFilters = repositoryFilters, accountIdOverride?: string) {
    const requestId = ++repositoryLoadSequence.current;
    setIsLoadingRepositories(true);
    const accountId = accountIdOverride ?? (authState.user ? String(authState.user.id) : undefined);

    if (!accountId) {
      if (requestId === repositoryLoadSequence.current) {
        setRepositoryPage(emptyRepositoryPage);
        setIsLoadingRepositories(false);
      }
      return;
    }

    try {
      let page = await loadRepositoryPage(accountId, nextFilters, 0);
      if (requestId === repositoryLoadSequence.current) {
        setRepositoryPage(page);
      }

      let nextOffset = page.offset + page.items.length;
      while (
        requestId === repositoryLoadSequence.current &&
        page.items.length > 0 &&
        nextOffset < page.totalCount
      ) {
        const nextPage = await loadRepositoryPage(accountId, nextFilters, nextOffset);
        if (requestId !== repositoryLoadSequence.current) {
          return;
        }
        if (nextPage.items.length === 0) {
          break;
        }
        page = {
          ...nextPage,
          items: [...page.items, ...nextPage.items],
          limit: REPOSITORY_PAGE_SIZE,
          offset: 0,
        };
        setRepositoryPage(page);
        nextOffset += nextPage.items.length;
      }
    } catch (reason) {
      if (requestId === repositoryLoadSequence.current) {
        setError(toErrorMessage(reason));
      }
    } finally {
      if (requestId === repositoryLoadSequence.current) {
        setIsLoadingRepositories(false);
      }
    }
  }

  async function loadRepositoryPage(accountId: string, filters: RepositoryFilters, offset: number) {
    return invoke<RepositoryListPage>('list_repositories', {
      request: {
        limit: REPOSITORY_PAGE_SIZE,
        offset,
        accountId,
        keyword: optionalRequestText(filters.keyword),
        language: optionalRequestText(filters.language),
        tagId: optionalRequestText(filters.tagId),
        readingStatus: optionalRequestText(filters.readingStatus),
      },
    });
  }

  async function loadRepositoryLanguages(accountIdOverride?: string) {
    try {
      const accountId = accountIdOverride ?? (authState.user ? String(authState.user.id) : undefined);
      if (!accountId) {
        setRepositoryLanguages([]);
        setRepositoryFilterCounts(emptyRepositoryFilterCounts);
        return;
      }
      const [languages, counts] = await Promise.all([
        invoke<string[]>('list_repository_languages', { request: { accountId } }),
        invoke<RepositoryFilterCounts>('get_repository_filter_counts', { request: { accountId } }),
      ]);
      setRepositoryLanguages(languages);
      setRepositoryFilterCounts(counts);
    } catch (reason) {
      setError(toErrorMessage(reason));
    }
  }

  async function loadTags(accountId = authState.user ? String(authState.user.id) : undefined) {
    try {
      if (!accountId) {
        setTags([]);
        return;
      }

      const nextTags = await invoke<TagItem[]>('list_tags', { request: { accountId } });
      setTags(nextTags);
    } catch (reason) {
      setError(toErrorMessage(reason));
    }
  }

  async function applyRepositoryFilters(nextFilters: RepositoryFilters) {
    setRepositoryFilters(nextFilters);
    await loadRepositories(nextFilters);
  }

  async function resetRepositoryFilters() {
    setRepositoryFilters(emptyRepositoryFilters);
    await loadRepositories(emptyRepositoryFilters);
  }

  async function refreshRepositoryWorkspace(accountIdOverride?: string) {
    await Promise.all([
      loadRepositories(repositoryFilters, accountIdOverride),
      loadRepositoryLanguages(accountIdOverride),
      loadTags(accountIdOverride),
      loadGithubRecommendationCandidates(accountIdOverride),
    ]);
  }

  async function loadGithubRecommendationCandidates(accountId = authState.user ? String(authState.user.id) : undefined) {
    if (!accountId) {
      setGithubRecommendationResponse(null);
      setGithubRecommendationError(null);
      return;
    }

    try {
      const page = await invoke<GithubRecommendationPage>('list_github_recommendation_candidates', {
        request: {
          accountId,
          limit: 12,
          offset: 0,
        },
      });
      setGithubRecommendationResponse(page.results.length > 0 ? {
        rationaleZh: page.rationaleZh,
        queries: page.queries,
        searchFailures: [],
        results: page.results,
      } : null);
      setGithubRecommendationError(null);
    } catch (reason) {
      setGithubRecommendationError('上次找到的候选没读出来。你的资料都还在。再找一次；还不行就重启应用，再不行把这条截图发给开发者。');
    }
  }

  function refreshRepositoryWorkspaceInBackground(accountId: string) {
    void refreshRepositoryWorkspace(accountId).catch((reason) => {
      setError('GitHub 已连接，但这台电脑上的资料没读出来。你的资料都还在。点「同步」再试；还不行就重启应用，再不行把这条截图发给开发者。');
    });
  }

  function setAiSearchRetryAction(action: (() => Promise<void>) | null) {
    lastAiSearchRetryRef.current = action;
  }

  async function retryLastAiSearch() {
    const action = lastAiSearchRetryRef.current;
    if (!action) {
      throw new Error('没有可重试的搜索。');
    }

    setIsRetryingAiSearch(true);
    try {
      await action();
    } finally {
      setIsRetryingAiSearch(false);
    }
  }

  async function loadAnnotationWorkspace(repository: RepositoryListItem) {
    const requestId = ++annotationLoadSequence.current;
    setIsLoadingAnnotation(true);
    setIsLoadingRepositoryDetail(true);
    setAnnotationMessage(null);

    try {
      const [nextTags, nextAnnotation, nextRepositoryDetail] = await Promise.all([
        invoke<TagItem[]>('list_tags', { request: { accountId: repository.accountId } }),
        invoke<RepositoryAnnotationView>('get_repository_annotation', {
          request: { repositoryId: repository.id, accountId: repository.accountId },
        }),
        invoke<RepositoryDetailView>('get_repository_detail', {
          request: { repositoryId: repository.id, accountId: repository.accountId },
        }),
      ]);
      if (requestId === annotationLoadSequence.current) {
        setTags(nextTags);
        setAnnotation(nextAnnotation);
        setRepositoryDetail(nextRepositoryDetail);
        setNoteDraft(nextAnnotation.noteMarkdown);
        setReadingStatusDraft(nextAnnotation.readingStatus);
      }
    } catch (reason) {
      if (requestId === annotationLoadSequence.current) {
        setError(toErrorMessage(reason));
      }
    } finally {
      if (requestId === annotationLoadSequence.current) {
        setIsLoadingAnnotation(false);
        setIsLoadingRepositoryDetail(false);
      }
    }
  }

  async function handleSaveToken(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSavingToken(true);
    setError(null);
    setAuthMessage(null);
    setTaskProgress(buildRunningTaskProgress(
      'connect-github',
      'auth',
      '正在验证 GitHub 令牌',
      null,
      'auth',
    ));

    try {
      const user = await invoke<GitHubUser>('save_github_token', { token });
      setAuthState({ hasToken: true, user });
      setToken('');
      refreshRepositoryWorkspaceInBackground(String(user.id));
      setAuthMessage('GitHub 账号已连接，可以同步收藏。');
      setTaskProgress(buildSucceededTaskProgress('connect-github', 'auth', `GitHub 账号 @${user.login} 已连接。`));
    } catch (reason) {
      const message = toErrorMessage(reason);
      setError(message);
      setTaskProgress(buildFailedTaskProgress('connect-github', 'auth', message));
    } finally {
      setIsSavingToken(false);
    }
  }

  /**
   * 直接用传入的 token 连接 GitHub，绕过 React state 异步更新问题。
   * 用于 WelcomeFlow 等需要立即用 token 调 invoke 的场景。
   */
  async function connectWithToken(rawToken: string) {
    const trimmed = rawToken.trim();
    if (!trimmed) {
      const message = '请输入 GitHub 令牌';
      setError(message);
      throw new Error(message);
    }
    setIsSavingToken(true);
    setError(null);
    setAuthMessage(null);
    setTaskProgress(buildRunningTaskProgress(
      'connect-github',
      'auth',
      '正在验证 GitHub 令牌',
      null,
      'auth',
    ));
    try {
      const user = await invoke<GitHubUser>('save_github_token', { token: trimmed });
      setAuthState({ hasToken: true, user });
      setToken('');
      refreshRepositoryWorkspaceInBackground(String(user.id));
      setAuthMessage('GitHub 账号已连接，可以同步收藏。');
      setTaskProgress(buildSucceededTaskProgress('connect-github', 'auth', `GitHub 账号 @${user.login} 已连接。`));
      return user;
    } catch (reason) {
      const message = toErrorMessage(reason);
      setError(message);
      setTaskProgress(buildFailedTaskProgress('connect-github', 'auth', message));
      throw reason;
    } finally {
      setIsSavingToken(false);
    }
  }

  async function handleClearToken() {
    setIsClearingToken(true);
    setError(null);
    setAuthMessage(null);

    try {
      await invoke('clear_github_token');
      resetWorkspaceAfterGitHubDisconnect();
      setAuthMessage('GitHub 连接已移除，这台电脑上的收藏不会被删除。');
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setIsClearingToken(false);
    }
  }

  async function handleClearLocalData() {
    setIsClearingLocalData(true);
    setError(null);
    setAuthMessage(null);

    try {
      const failures: string[] = [];
      let didClearToken = false;
      let didClearDatabase = false;

      try {
        await invoke('clear_github_token');
        didClearToken = true;
      } catch (reason) {
        failures.push(`GitHub 令牌没能清掉：${toErrorMessage(reason)}`);
      }

      try {
        await invoke('clear_local_database');
        didClearDatabase = true;
      } catch (reason) {
        failures.push(`本机资料没能清掉：${toErrorMessage(reason)}`);
      }

      if (didClearToken) {
        setAuthState(initialAuthState);
      }
      setToken('');
      setSyncSummary(null);
      setLastSyncedAt(null);
      setReadmeSummary(null);
      setBatchAiSummary(null);
      setGithubRecommendationResponse(null);
      setGithubRecommendationError(null);
      setLastGithubRecommendationRepositoryIds([]);

      if (didClearDatabase) {
        setRepositoryPage(emptyRepositoryPage);
        setRepositoryLanguages([]);
        setRepositoryFilterCounts(emptyRepositoryFilterCounts);
        setRepositoryFilters(emptyRepositoryFilters);
        setSelectedRepositoryId(null);
        setTags([]);
        setAnnotation(null);
        setRepositoryDetail(null);
        setNoteDraft('');
        setReadingStatusDraft('unread');
        setGistIdDraft('');
        setRepositoryReadmeError(null);
        setRepositoryAiError(null);
      }

      if (failures.length > 0) {
        throw new Error(failures.join('；'));
      }

      setAuthMessage('本机资料已清空，应用已回到刚安装的样子。');
      setTaskProgress(buildSucceededTaskProgress('clear-local-data', 'storage', '本机资料已清空。'));
    } catch (reason) {
      const message = toErrorMessage(reason);
      setError(`本机资料没能清空。你在 GitHub 上的收藏不受影响。${message}`);
      setTaskProgress(buildFailedTaskProgress('clear-local-data', 'storage', message));
      throw reason;
    } finally {
      setIsClearingLocalData(false);
    }
  }

  async function refreshLastSyncedAt() {
    try {
      const view = await getTodayView();
      setLastSyncedAt(view.syncedAt);
    } catch {
      // 读不到就留着上次成功的时间。从没读到过时仍是空，顶栏写「还没同步过」。
    }
  }

  async function handleSyncStars(options?: { throwOnError?: boolean }) {
    setIsSyncingStars(true);
    setError(null);
    setAuthMessage(null);
    setTaskProgress(buildRunningTaskProgress('sync-stars', 'sync', '正在准备同步收藏'));

    try {
      const summary = await invoke<StarSyncSummary>('sync_github_stars');
      setSyncSummary(summary);
      setReadmeSummary(null);
      await refreshRepositoryWorkspace();
      await refreshLastSyncedAt();
      setAuthMessage(formatSyncComplete(summary));
    } catch (reason) {
      const message = toErrorMessage(reason);
      const displayMessage = '同步没完成。你的资料都还在。点「重试」；还不行就重启应用，再不行把这条截图发给开发者。';
      setError(displayMessage);
      setTaskProgress(buildFailedTaskProgress('sync-stars', 'sync', displayMessage));
      if (isGitHubConnectionUnavailableError(message)) {
        resetWorkspaceAfterGitHubDisconnect();
        setAuthMessage('令牌已失效，已断开连接。请重新输入令牌后再同步。');
      } else {
        await refreshRepositoryWorkspace();
      }
      if (options?.throwOnError) {
        throw new Error(displayMessage);
      }
    } finally {
      setIsSyncingStars(false);
    }
  }

  function resetWorkspaceAfterGitHubDisconnect() {
    setAuthState(initialAuthState);
    setToken('');
    setSyncSummary(null);
    setReadmeSummary(null);
    setBatchAiSummary(null);
    setGithubRecommendationResponse(null);
    setGithubRecommendationError(null);
    setLastGithubRecommendationRepositoryIds([]);
    setRepositoryPage(emptyRepositoryPage);
    setRepositoryLanguages([]);
    setRepositoryFilters(emptyRepositoryFilters);
    setSelectedRepositoryId(null);
    setTags([]);
    setAnnotation(null);
    setRepositoryDetail(null);
    setNoteDraft('');
    setReadingStatusDraft('unread');
    setGistIdDraft('');
    setRepositoryReadmeError(null);
    setRepositoryAiError(null);
  }

  function handleGitHubCredentialFailure(message: string) {
    if (!isGitHubConnectionUnavailableError(message)) {
      return false;
    }

    resetWorkspaceAfterGitHubDisconnect();
    setAuthMessage('令牌已失效，已断开连接。请重新输入令牌后再继续。');
    return true;
  }

  async function handleFetchReadmes(options?: {
    aiConfig?: AISettings;
    autoGenerateAi?: boolean;
    aiLimit?: number;
    onlyMissing?: boolean;
    repositoryIds?: string[];
    throwOnFailure?: boolean;
  }) {
    setIsFetchingReadmes(true);
    setError(null);
    setAuthMessage(null);
    setTaskProgress(buildRunningTaskProgress('fetch-readmes', 'readme', '正在准备下载项目说明'));

    try {
      const summary = await invoke<ReadmeFetchSummary>('fetch_repository_readmes', {
        request: {
          onlyMissing: options?.onlyMissing ?? true,
          repositoryIds: options?.repositoryIds,
        },
      });
      setReadmeSummary(summary);
      await refreshRepositoryWorkspace();
      const readmeMessage = `项目说明已处理 ${summary.totalCount} 个：更新 ${summary.fetchedCount}，跳过 ${summary.skippedCount}，缺失 ${summary.missingCount}，失败 ${summary.failedCount}。`;
      setAuthMessage(readmeMessage);
      const readmeFailureMessage = buildReadmeFailureMessage(summary);
      if (readmeFailureMessage) {
        setError(readmeFailureMessage);
        setTaskProgress(buildPartialTaskProgress(
          'fetch-readmes',
          'readme',
          readmeFailureMessage,
          summary.totalCount,
        ));
        if (options?.throwOnFailure) {
          throw new ReadmePostProcessWarning(readmeFailureMessage);
        }
      }

      if (options?.autoGenerateAi) {
        const aiConfig = options.aiConfig;
        if (!aiConfig) {
          const message = '请先到「设置 → AI 设置」选一个服务商。';
          const warningMessage = `项目说明已下载，但还没开始整理：${message}`;
          setError(message);
          setTaskProgress(buildFailedTaskProgress('batch-generate-ai-documents', 'ai', message));
          setAuthMessage(`${readmeMessage} 还没开始整理：${message}`);
          if (options.throwOnFailure) {
            throw new ReadmePostProcessWarning(warningMessage);
          }
          return summary;
        }

        const aiConfigMessage = getAiConfigMessage(aiConfig);
        if (aiConfigMessage) {
          const warningMessage = `项目说明已下载，但还没开始整理：${aiConfigMessage}`;
          setError(aiConfigMessage);
          setTaskProgress(buildFailedTaskProgress('batch-generate-ai-documents', 'ai', aiConfigMessage));
          setAuthMessage(`${readmeMessage} 还没开始整理：${aiConfigMessage}`);
          if (options.throwOnFailure) {
            throw new ReadmePostProcessWarning(warningMessage);
          }
          return summary;
        }

        try {
          await handleBatchGenerateAiDocuments(aiConfig, {
            limit: options.aiLimit,
            onlyMissing: options.onlyMissing ?? true,
            repositoryIds: options.repositoryIds,
          });
        } catch (reason) {
          const message = toErrorMessage(reason);
          const warningMessage = `项目说明已下载，但这次没整理成功。你的资料都还在。点「重试」；还不行就重启应用，再不行把这条截图发给开发者。`;
          setError(warningMessage);
          setAuthMessage(`${readmeMessage} 这次没整理成功。你的资料都还在。`);
          if (options.throwOnFailure) {
            throw new ReadmePostProcessWarning(warningMessage);
          }
        }
      }

      return summary;
    } catch (reason) {
      const message = toErrorMessage(reason);
      setError(message);
      if (!(reason instanceof ReadmePostProcessWarning)) {
        setTaskProgress(buildFailedTaskProgress('fetch-readmes', 'readme', message));
      }
      handleGitHubCredentialFailure(message);
      if (options?.throwOnFailure) {
        throw new Error(message);
      }
      return null;
    } finally {
      setIsFetchingReadmes(false);
    }
  }

  async function handleExportAnnotations() {
    setIsExportingAnnotations(true);
    setError(null);
    setAuthMessage(null);
    setTaskProgress(buildRunningTaskProgress(
      'export-annotation-gist',
      'backup',
      '正在把标记上传到 GitHub（私密 Gist）',
      'GitHub Gist',
      'save',
    ));

    try {
      const summary = await invoke<GistAnnotationExportSummary>('export_annotation_gist');
      setGistIdDraft(summary.gistId);
      setAuthMessage(`标记已上传到 GitHub（私密 Gist ${summary.gistId}）：${summary.tagCount} 个标签，${summary.repositoryCount} 条工具的标记。`);
      setTaskProgress(buildSucceededTaskProgress('export-annotation-gist', 'backup', '标记已上传到 GitHub。'));
    } catch (reason) {
      const message = toErrorMessage(reason);
      setError(message);
      setTaskProgress(buildFailedTaskProgress('export-annotation-gist', 'backup', message, 'GitHub Gist'));
      handleGitHubCredentialFailure(message);
    } finally {
      setIsExportingAnnotations(false);
    }
  }

  async function handleExportRepositoryLibrary() {
    setIsExportingRepositoryLibrary(true);
    setError(null);
    setAuthMessage(null);
    setTaskProgress(buildRunningTaskProgress(
      'export-repository-library-gist',
      'backup',
      '正在把整个清单上传到 GitHub（私密 Gist）',
      'GitHub Gist',
      'save',
    ));

    try {
      const summary = await invoke<GistRepositoryLibraryExportSummary>('export_repository_library_gist');
      setGistIdDraft(summary.gistId);
      setAuthMessage(`清单已上传到 GitHub（私密 Gist ${summary.gistId}）：${summary.repositoryCount} 个工具，${summary.tagCount} 个标签。`);
      setTaskProgress(buildSucceededTaskProgress('export-repository-library-gist', 'backup', '整个清单已上传到 GitHub。'));
    } catch (reason) {
      const message = toErrorMessage(reason);
      setError(message);
      setTaskProgress(buildFailedTaskProgress('export-repository-library-gist', 'backup', message, 'GitHub Gist'));
      handleGitHubCredentialFailure(message);
    } finally {
      setIsExportingRepositoryLibrary(false);
    }
  }

  async function handleImportAnnotations(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await runImportAnnotations();
  }

  async function runImportAnnotations() {
    const gistId = gistIdDraft.trim();

    if (!gistId) {
      const message = '请输入 Gist ID 后再导入。';
      setError(message);
      setAuthMessage(null);
      setTaskProgress(buildFailedTaskProgress('import-annotation-gist', 'backup', message, 'GitHub Gist'));
      return;
    }

    setIsImportingAnnotations(true);
    setError(null);
    setAuthMessage(null);
    setTaskProgress(buildRunningTaskProgress(
      'import-annotation-gist',
      'backup',
      '正在从 GitHub（私密 Gist）导入标记',
      `Gist ${gistId}`,
      'fetch',
    ));

    try {
      const summary = await invoke<GistAnnotationImportSummary>('import_annotation_gist', {
        request: { gistId },
      });
      await refreshRepositoryWorkspace();
      if (selectedRepository) {
        await loadAnnotationWorkspace(selectedRepository);
      }
      setAuthMessage(
        `标记已导入：${summary.tagCount} 个标签，${summary.repositoryCount} 条工具的标记，跳过 ${summary.skippedRepositoryCount} 条这台电脑上没有、或你已取消收藏的工具。`,
      );
      setTaskProgress(buildSucceededTaskProgress('import-annotation-gist', 'backup', '标记已从 GitHub 导入。'));
    } catch (reason) {
      const message = toErrorMessage(reason);
      setError(message);
      setTaskProgress(buildFailedTaskProgress('import-annotation-gist', 'backup', message, `Gist ${gistId}`));
      handleGitHubCredentialFailure(message);
    } finally {
      setIsImportingAnnotations(false);
    }
  }

  async function handleImportRepositoryLibrary(event?: { preventDefault: () => void }) {
    event?.preventDefault();
    const gistId = gistIdDraft.trim();

    if (!gistId) {
      const message = '请输入 Gist ID 后再导入。';
      setError(message);
      setAuthMessage(null);
      setTaskProgress(buildFailedTaskProgress('import-repository-library-gist', 'backup', message, 'GitHub Gist'));
      return;
    }

    setIsImportingRepositoryLibrary(true);
    setError(null);
    setAuthMessage(null);
    setTaskProgress(buildRunningTaskProgress(
      'import-repository-library-gist',
      'backup',
      '正在从 GitHub（私密 Gist）导入整个清单',
      `Gist ${gistId}`,
      'fetch',
    ));

    try {
      const summary = await invoke<GistRepositoryLibraryImportSummary>('import_repository_library_gist', {
        request: { gistId },
      });
      await refreshRepositoryWorkspace();
      if (selectedRepository) {
        await loadAnnotationWorkspace(selectedRepository);
      }
      setAuthMessage(
        `清单已导入：新增 ${summary.createdRepositoryCount} 个工具，更新 ${summary.updatedRepositoryCount} 个工具，恢复 ${summary.tagCount} 个标签。`,
      );
      setTaskProgress(buildSucceededTaskProgress('import-repository-library-gist', 'backup', '整个清单已从 GitHub 导入。'));
    } catch (reason) {
      const message = toErrorMessage(reason);
      setError(message);
      setTaskProgress(buildFailedTaskProgress('import-repository-library-gist', 'backup', message, `Gist ${gistId}`));
      handleGitHubCredentialFailure(message);
    } finally {
      setIsImportingRepositoryLibrary(false);
    }
  }

  async function handleCreateTag(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!selectedRepository || newTagName.trim().length === 0) {
      return;
    }

    setIsSavingTag(true);
    setError(null);
    setAnnotationMessage(null);

    try {
      const createdTag = await invoke<TagItem>('create_tag', {
        request: {
          accountId: selectedRepository.accountId,
          name: newTagName,
          color: newTagColor,
        },
      });
      const currentTagIds = new Set(annotation?.tags.map((item) => item.id) ?? []);
      currentTagIds.add(createdTag.id);
      const nextAnnotation = await invoke<RepositoryAnnotationView>('set_repository_tags', {
        request: {
          repositoryId: selectedRepository.id,
          accountId: selectedRepository.accountId,
          tagIds: Array.from(currentTagIds),
        },
      });
      setTags((currentTags) => {
        const withoutDuplicate = currentTags.filter((tag) => tag.id !== createdTag.id);
        return [...withoutDuplicate, createdTag].sort((left, right) => left.name.localeCompare(right.name));
      });
      setAnnotation(nextAnnotation);
      await loadRepositories(repositoryFilters);
      setNewTagName('');
      setAnnotationMessage(`标签「${createdTag.name}」已创建，并标到当前工具上。`);
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setIsSavingTag(false);
    }
  }

  async function handleRenameTag(tag: TagItem, nextName: string) {
    if (!selectedRepository) {
      return;
    }

    const normalizedName = nextName.trim();

    if (!normalizedName || normalizedName === tag.name) {
      return;
    }

    setIsSavingTag(true);
    setError(null);
    setAnnotationMessage(null);

    try {
      const nextTag = await invoke<TagItem>('update_tag', {
        request: {
          accountId: selectedRepository.accountId,
          tagId: tag.id,
          name: normalizedName,
          color: tag.color,
        },
      });
      setTags((currentTags) => currentTags.map((item) => (item.id === nextTag.id ? nextTag : item)));
      setAnnotation((currentAnnotation) =>
        currentAnnotation
          ? {
              ...currentAnnotation,
              tags: currentAnnotation.tags.map((item) => (item.id === nextTag.id ? nextTag : item)),
            }
          : currentAnnotation,
      );
      await loadRepositories(repositoryFilters);
      setAnnotationMessage('标签已重命名。');
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setIsSavingTag(false);
    }
  }

  async function handleDeleteTag(tag: TagItem) {
    if (!selectedRepository) {
      return;
    }

    setIsSavingTag(true);
    setError(null);
    setAnnotationMessage(null);

    try {
      await invoke('delete_tag', {
        request: {
          accountId: selectedRepository.accountId,
          tagId: tag.id,
        },
      });
      setTags((currentTags) => currentTags.filter((item) => item.id !== tag.id));
      if (repositoryFilters.tagId === tag.id) {
        await resetRepositoryFilters();
      } else {
        await loadRepositories(repositoryFilters);
      }
      setAnnotation((currentAnnotation) =>
        currentAnnotation
          ? { ...currentAnnotation, tags: currentAnnotation.tags.filter((item) => item.id !== tag.id) }
          : currentAnnotation,
      );
      setAnnotationMessage('标签已删除。');
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setIsSavingTag(false);
    }
  }

  async function handleToggleRepositoryTag(tag: TagItem) {
    if (!selectedRepository || !annotation) {
      return;
    }

    setIsSavingTag(true);
    setError(null);
    setAnnotationMessage(null);

    const currentTagIds = new Set(annotation.tags.map((item) => item.id));

    if (currentTagIds.has(tag.id)) {
      currentTagIds.delete(tag.id);
    } else {
      currentTagIds.add(tag.id);
    }

    try {
      const nextAnnotation = await invoke<RepositoryAnnotationView>('set_repository_tags', {
        request: {
          repositoryId: selectedRepository.id,
          accountId: selectedRepository.accountId,
          tagIds: Array.from(currentTagIds),
        },
      });
      setAnnotation(nextAnnotation);
      await loadRepositories(repositoryFilters);
      setAnnotationMessage('这个工具的标签已更新。');
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setIsSavingTag(false);
    }
  }

  async function handleApplySuggestedTag(tagName: string) {
    if (!selectedRepository || !annotation) {
      return;
    }

    const normalizedTagName = tagName.trim();
    if (!normalizedTagName) {
      return;
    }

    setIsSavingTag(true);
    setError(null);
    setAnnotationMessage(null);

    try {
      let targetTag = tags.find((tag) => tag.name.toLowerCase() === normalizedTagName.toLowerCase()) ?? null;

      if (!targetTag) {
        const createdTag = await invoke<TagItem>('create_tag', {
          request: {
            accountId: selectedRepository.accountId,
            name: normalizedTagName,
            color: getSuggestedTagColor(normalizedTagName),
          },
        });
        targetTag = createdTag;
        setTags((currentTags) => [...currentTags, createdTag].sort((left, right) => left.name.localeCompare(right.name)));
      }

      const currentTagIds = new Set(annotation.tags.map((item) => item.id));
      if (currentTagIds.has(targetTag.id)) {
        setAnnotationMessage(`标签「${targetTag.name}」已标到当前工具上。`);
        return;
      }

      currentTagIds.add(targetTag.id);
      const nextAnnotation = await invoke<RepositoryAnnotationView>('set_repository_tags', {
        request: {
          repositoryId: selectedRepository.id,
          accountId: selectedRepository.accountId,
          tagIds: Array.from(currentTagIds),
        },
      });
      setAnnotation(nextAnnotation);
      await loadRepositories(repositoryFilters);
      setAnnotationMessage(`标签「${targetTag.name}」已标到当前工具上。`);
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setIsSavingTag(false);
    }
  }

  async function handleSaveAnnotation() {
    if (!selectedRepository) {
      return;
    }

    setIsSavingAnnotation(true);
    setError(null);
    setAnnotationMessage(null);

    try {
      const nextAnnotation = await invoke<RepositoryAnnotationView>('save_repository_annotation', {
        request: {
          repositoryId: selectedRepository.id,
          accountId: selectedRepository.accountId,
          noteMarkdown: noteDraft,
          readingStatus: readingStatusDraft,
        },
      });
      setAnnotation(nextAnnotation);
      setNoteDraft(nextAnnotation.noteMarkdown);
      setReadingStatusDraft(nextAnnotation.readingStatus);
      await loadRepositories(repositoryFilters);
      setAnnotationMessage('笔记和我的状态已保存。');
    } catch (reason) {
      setError(toErrorMessage(reason));
    } finally {
      setIsSavingAnnotation(false);
    }
  }

  async function handleFetchRepositoryReadme() {
    if (!selectedRepository) {
      return;
    }

    setIsFetchingRepositoryReadme(true);
    setError(null);
    setRepositoryReadmeError(null);
    setAnnotationMessage(null);
    setTaskProgress(buildRunningTaskProgress(
      'fetch-repository-readme',
      'readme',
      `正在准备下载 ${selectedRepository.fullName} 的项目说明`,
      selectedRepository.fullName,
    ));

    try {
      const nextRepositoryDetail = await invoke<RepositoryDetailView>('fetch_repository_readme', {
        request: {
          repositoryId: selectedRepository.id,
          accountId: selectedRepository.accountId,
        },
      });
      setRepositoryDetail(nextRepositoryDetail);
      await refreshRepositoryWorkspace();
      setRepositoryReadmeError(null);
      setAnnotationMessage('项目说明已下载。');
    } catch (reason) {
      const message = toErrorMessage(reason);
      setError(message);
      setRepositoryReadmeError({ repositoryId: selectedRepository.id, message });
      setTaskProgress(buildFailedTaskProgress('fetch-repository-readme', 'readme', message, selectedRepository.fullName));
      handleGitHubCredentialFailure(message);
      throw reason;
    } finally {
      setIsFetchingRepositoryReadme(false);
    }
  }

  async function handleGenerateAiDocument(aiConfig: AISettings) {
    if (!selectedRepository) {
      return;
    }

    const aiConfigMessage = getAiConfigMessage(aiConfig);
    if (aiConfigMessage) {
      setError(aiConfigMessage);
      setRepositoryAiError({ repositoryId: selectedRepository.id, message: aiConfigMessage });
      setTaskProgress(buildFailedTaskProgress('generate-ai-document', 'ai', aiConfigMessage, selectedRepository.fullName));
      throw new Error(aiConfigMessage);
    }

    setIsGeneratingAiDocument(true);
    setError(null);
    setRepositoryAiError(null);
    setAnnotationMessage(null);
    const requestId = `repository-ai-${selectedRepository.id}-${Date.now()}`;
    const now = new Date().toISOString();
    setRepositoryAiStream({
      requestId,
      repositoryId: selectedRepository.id,
      stage: 'prepare',
      status: 'started',
      message: `正在准备整理 ${selectedRepository.fullName}`,
      text: '',
      startedAt: now,
      updatedAt: now,
    });
    setTaskProgress(buildRunningTaskProgress(
      'generate-ai-document',
      'ai',
      `正在准备整理 ${selectedRepository.fullName}`,
      selectedRepository.fullName,
    ));

    try {
      const nextRepositoryDetail = await invoke<RepositoryDetailView>('generate_repository_ai_document', {
        request: {
          repositoryId: selectedRepository.id,
          accountId: selectedRepository.accountId,
          aiConfig: toBackendAiRequestConfig(aiConfig),
          requestId,
        },
      });
      setRepositoryDetail(nextRepositoryDetail);
      await refreshRepositoryWorkspace();
      setRepositoryAiError(null);
      setRepositoryAiStream((current) => current?.requestId === requestId
        ? { ...current, status: 'finished', stage: 'done', message: '中文说明已写好。', updatedAt: new Date().toISOString() }
        : current);
      setAnnotationMessage('中文说明已写好。');
    } catch (reason) {
      const message = toErrorMessage(reason);
      setError(message);
      setRepositoryAiError({ repositoryId: selectedRepository.id, message });
      setRepositoryAiStream((current) => current?.requestId === requestId
        ? { ...current, status: 'failed', stage: 'error', message, updatedAt: new Date().toISOString() }
        : current);
      setTaskProgress(buildFailedTaskProgress('generate-ai-document', 'ai', message, selectedRepository.fullName));
      handleGitHubCredentialFailure(message);
      throw reason;
    } finally {
      setIsGeneratingAiDocument(false);
    }
  }

  async function handleBatchGenerateAiDocuments(aiConfig: AISettings, options?: { limit?: number; onlyMissing?: boolean; repositoryIds?: string[] }) {
    const aiConfigMessage = getAiConfigMessage(aiConfig);
    if (aiConfigMessage) {
      setError(aiConfigMessage);
      setTaskProgress(buildFailedTaskProgress('batch-generate-ai-documents', 'ai', aiConfigMessage));
      throw new Error(aiConfigMessage);
    }

    setIsBatchGeneratingAiDocuments(true);
    setError(null);
    setAuthMessage(null);
    setTaskProgress(buildRunningTaskProgress('batch-generate-ai-documents', 'ai', '正在检查还没有中文说明的收藏'));

    try {
      const summary = await invoke<BatchAiDocumentSummary>('batch_generate_repository_ai_documents', {
        request: {
          aiConfig: toBackendAiRequestConfig(aiConfig),
          limit: options?.limit,
          onlyMissing: options?.onlyMissing ?? true,
          repositoryIds: options?.repositoryIds,
        },
      });
      setBatchAiSummary(summary);
      await refreshRepositoryWorkspace();
      if (selectedRepository) {
        await loadAnnotationWorkspace(selectedRepository);
      }
      setAuthMessage(summary.totalCount === 0
        ? '中文说明已经是最新的，不用再整理。'
        : `整理完成：写好 ${summary.generatedCount} 个，跳过 ${summary.skippedCount} 个，还没有项目说明 ${summary.missingReadmeCount} 个，失败 ${summary.failedCount} 个。`);
      const batchAiFailureMessage = buildBatchAiFailureMessage(summary);
      if (batchAiFailureMessage) {
        setError(batchAiFailureMessage);
        setTaskProgress(buildPartialTaskProgress(
          'batch-generate-ai-documents',
          'ai',
          batchAiFailureMessage,
          summary.totalCount,
        ));
      }
      return summary;
    } catch (reason) {
      const message = toErrorMessage(reason);
      setError(message);
      setTaskProgress(buildFailedTaskProgress('batch-generate-ai-documents', 'ai', message));
      handleGitHubCredentialFailure(message);
      throw reason;
    } finally {
      setIsBatchGeneratingAiDocuments(false);
    }
  }

  async function handleGenerateAiTagNetwork(aiConfig: AISettings) {
    const accountId = authState.user ? String(authState.user.id) : null;
    if (!accountId) {
      const message = '请先在设置中连接 GitHub 账号。';
      setError(message);
      setTaskProgress(buildFailedTaskProgress('generate-ai-tag-network', 'ai', message));
      throw new Error(message);
    }

    const aiConfigMessage = getAiConfigMessage(aiConfig);
    if (aiConfigMessage) {
      setError(aiConfigMessage);
      setTaskProgress(buildFailedTaskProgress('generate-ai-tag-network', 'ai', aiConfigMessage));
      throw new Error(aiConfigMessage);
    }

    setIsGeneratingTagNetwork(true);
    setError(null);
    setAuthMessage(null);
    setTaskProgress(buildRunningTaskProgress('generate-ai-tag-network', 'ai', '正在准备整理标签'));

    try {
      const summary = await invoke<AiTagNetworkSummary>('generate_ai_tag_network', {
        request: {
          accountId,
          aiConfig: toBackendAiRequestConfig(aiConfig),
        },
      });
      await refreshRepositoryWorkspace();
      const partialFailureMessage = summary.failedBatchCount > 0
        ? `其中 ${summary.failedBatchCount} 批没成功，已经写好的标签还在，可以稍后再试。`
        : '';
      setAuthMessage(`标签已整理：${summary.tagCount} 个标签，关联了 ${summary.linkedCount} 个工具。${partialFailureMessage}`);
      return summary;
    } catch (reason) {
      const message = toErrorMessage(reason);
      setError(message);
      setTaskProgress(buildFailedTaskProgress('generate-ai-tag-network', 'ai', message));
      handleGitHubCredentialFailure(message);
      throw reason;
    } finally {
      setIsGeneratingTagNetwork(false);
    }
  }

  async function handleFindSimilarRepositories(aiConfig: AISettings, repositoryIds: string[], options?: { limit?: number }) {
    const accountId = authState.user ? String(authState.user.id) : null;
    const selectedRepositoryIds = repositoryIds.slice(0, 8);
    if (!accountId) {
      const message = '请先连接 GitHub 账号。';
      setGithubRecommendationError(message);
      setTaskProgress(buildFailedTaskProgress('recommend-github-repositories', 'ai', message));
      throw new Error(message);
    }
    if (selectedRepositoryIds.length === 0) {
      const message = '请先勾选 1 到 8 个工具作为参考。';
      setGithubRecommendationError(message);
      setTaskProgress(buildFailedTaskProgress('recommend-github-repositories', 'ai', message));
      throw new Error(message);
    }
    setLastGithubRecommendationRepositoryIds(selectedRepositoryIds);

    const aiConfigMessage = getAiConfigMessage(aiConfig);
    if (aiConfigMessage) {
      setGithubRecommendationError(aiConfigMessage);
      setTaskProgress(buildFailedTaskProgress('recommend-github-repositories', 'ai', aiConfigMessage));
      throw new Error(aiConfigMessage);
    }

    setIsFindingSimilarRepositories(true);
    setGithubRecommendationError(null);
    setTaskProgress(buildRunningTaskProgress(
      'recommend-github-repositories',
      'ai',
      `正在根据 ${selectedRepositoryIds.length} 个工具准备相似发现`,
    ));

    try {
      const response = await invoke<GithubRecommendationResponse>('recommend_github_repositories', {
        request: {
          accountId,
          repositoryIds: selectedRepositoryIds,
          aiConfig: toBackendAiRequestConfig(aiConfig),
          limit: options?.limit ?? 12,
        },
      });
      setGithubRecommendationResponse(response);
      setGithubRecommendationError(null);
      return response;
    } catch (reason) {
      const message = toErrorMessage(reason);
      setGithubRecommendationError(message);
      setTaskProgress(buildFailedTaskProgress('recommend-github-repositories', 'ai', message));
      handleGitHubCredentialFailure(message);
      throw reason;
    } finally {
      setIsFindingSimilarRepositories(false);
    }
  }

  async function handleUpdateRecommendationCandidate(fullName: string, status: 'marked' | 'ignored' | 'new') {
    const accountId = authState.user ? String(authState.user.id) : null;
    if (!accountId) {
      const message = '请先连接 GitHub 账号。';
      setGithubRecommendationError(message);
      throw new Error(message);
    }

    try {
      const candidate = await invoke<{ id: string; fullName: string; status: 'new' | 'marked' | 'ignored' | 'starred' }>(
        'update_github_recommendation_candidate_status',
        {
          request: {
            accountId,
            fullName,
            status,
          },
        },
      );
      setGithubRecommendationResponse((current) => updateRecommendationCandidateState(current, candidate));
      setGithubRecommendationError(null);
    } catch (reason) {
      setGithubRecommendationError(toErrorMessage(reason));
      throw reason;
    }
  }

  async function handleStarRecommendationCandidate(fullName: string) {
    const accountId = authState.user ? String(authState.user.id) : null;
    setLastStarRecommendationCandidateFullName(fullName);
    if (!accountId) {
      const message = '请先连接 GitHub 账号。';
      setGithubRecommendationError(message);
      setTaskProgress(buildFailedTaskProgress('star-github-recommendation-candidate', 'sync', message, fullName));
      throw new Error(message);
    }

    setTaskProgress(buildRunningTaskProgress(
      'star-github-recommendation-candidate',
      'sync',
      `正在把 ${fullName} 加到 GitHub 收藏`,
      fullName,
      'github-star',
    ));
    setIsStarringRecommendationCandidate(true);
    let candidate: { id: string; fullName: string; status: 'new' | 'marked' | 'ignored' | 'starred' };
    try {
      candidate = await invoke<{ id: string; fullName: string; status: 'new' | 'marked' | 'ignored' | 'starred' }>(
        'star_github_recommendation_candidate',
        {
          request: {
            accountId,
            fullName,
          },
        },
      );
    } catch (reason) {
      const message = toErrorMessage(reason);
      setGithubRecommendationError(message);
      setTaskProgress(buildFailedTaskProgress('star-github-recommendation-candidate', 'sync', message, fullName));
      handleGitHubCredentialFailure(message);
      throw reason;
    } finally {
      setIsStarringRecommendationCandidate(false);
    }

    setGithubRecommendationResponse((current) => updateRecommendationCandidateState(current, candidate));
    setGithubRecommendationError(null);
    setTaskProgress(buildSucceededTaskProgress(
      'star-github-recommendation-candidate',
      'sync',
      `${candidate.fullName} 已在 GitHub 上收藏，正在同步这台电脑上的资料。`,
    ));
    await handleSyncStars().catch((reason) => {
      const message = '已在 GitHub 上收藏，但这台电脑还没同步完。你的资料都还在。点「重试」；还不行就重启应用，再不行把这条截图发给开发者。';
      setError(message);
    });
  }

  return {
    annotation,
    annotationMessage,
    applyRepositoryFilters,
    authMessage,
    authState,
    batchAiSummary,
    connectWithToken,
    error,
    gistIdDraft,
    githubRecommendationError,
    githubRecommendationResponse,
    handleClearToken,
    handleClearLocalData,
    handleCreateTag,
    handleDeleteTag,
    handleExportAnnotations,
    handleExportRepositoryLibrary,
    handleFetchReadmes,
    handleFetchRepositoryReadme,
    handleBatchGenerateAiDocuments,
    handleFindSimilarRepositories,
    handleGenerateAiTagNetwork,
    handleGenerateAiDocument,
    handleApplySuggestedTag,
    handleImportAnnotations,
    handleImportRepositoryLibrary,
    runImportAnnotations,
    handleRenameTag,
    handleSaveAnnotation,
    handleSaveToken,
    handleSyncStars,
    handleStarRecommendationCandidate,
    handleToggleRepositoryTag,
    handleUpdateRecommendationCandidate,
    isClearingToken,
    isClearingLocalData,
    isExportingAnnotations,
    isExportingRepositoryLibrary,
    isFetchingReadmes,
    isFetchingRepositoryReadme,
    isBatchGeneratingAiDocuments,
    isFindingSimilarRepositories,
    isGeneratingAiDocument,
    isGeneratingTagNetwork,
    isImportingAnnotations,
    isImportingRepositoryLibrary,
    isLoadingAuth,
    isLoadingAnnotation,
    isLoadingRepositories,
    isLoadingRepositoryDetail,
    isSavingAnnotation,
    isSavingTag,
    isSavingToken,
    isRetryingAiSearch,
    isStarringRecommendationCandidate,
    isSyncingStars,
    lastGithubRecommendationRepositoryIds,
    lastStarRecommendationCandidateFullName,
    newTagColor,
    newTagName,
    noteDraft,
    readingStatusDraft,
    readmeSummary,
    refreshRepositoryWorkspace,
    repositoryDetail,
    repositoryFilters,
    repositoryAiError,
    repositoryAiStream,
    repositoryReadmeError,
    repositoryLanguages,
    repositoryFilterCounts,
    repositoryPage,
    repositoryStats,
    resetRepositoryFilters,
    selectedRepository,
    setGistIdDraft,
    setNewTagColor,
    setNewTagName,
    setNoteDraft,
    setReadingStatusDraft,
    setAiSearchRetryAction,
    setSelectedRepositoryId,
    setToken,
    showTaskProgress: setTaskProgress,
    status,
    syncSummary,
    lastSyncedAt,
    tags,
    taskProgress,
    token,
    retryLastAiSearch,
  };
}

function toErrorMessage(reason: unknown) {
  const raw = reason instanceof Error ? reason.message : String(reason);
  return plainTechnicalError(raw) ?? raw;
}

function isGitHubConnectionUnavailableError(message: string) {
  return (
    message.includes('请先连接 GitHub 账号')
    || message.includes('Token 无效或权限不足')
    || message.includes('HTTP 401')
  );
}

function updateRecommendationCandidateState(
  current: GithubRecommendationResponse | null,
  candidate: { id: string; fullName: string; status: 'new' | 'marked' | 'ignored' | 'starred' },
) {
  if (!current) {
    return current;
  }

  return {
    ...current,
    results: current.results.map((repository) => repository.fullName === candidate.fullName
      ? {
          ...repository,
          candidateId: candidate.id,
          candidateStatus: candidate.status,
        }
      : repository),
  };
}

function buildReadmeFailureMessage(summary: ReadmeFetchSummary) {
  if (summary.failedCount <= 0) {
    return null;
  }

  return `有 ${summary.failedCount} 个项目说明没读出来。你的资料都还在。点「重试」；还不行就重启应用，再不行把这条截图发给开发者。`;
}

function buildBatchAiFailureMessage(summary: BatchAiDocumentSummary) {
  if (summary.failedCount <= 0) {
    return null;
  }

  return `有 ${summary.failedCount} 个没整理成功，已整理好的都保留着。可以稍后再点一次，或到「设置 → AI 设置」换一个能处理长文章的模型。`;
}

function buildFailedTaskProgress(
  taskId: string,
  taskType: string,
  message: string,
  repositoryName: string | null = null,
): TaskProgressEvent {
  return {
    taskId,
    taskType,
    status: 'failed',
    stage: 'error',
    current: 0,
    total: 0,
    message,
    repositoryName,
  };
}

function buildPartialTaskProgress(
  taskId: string,
  taskType: string,
  message: string,
  totalCount: number,
): TaskProgressEvent {
  const safeTotal = Math.max(1, totalCount);
  return {
    taskId,
    taskType,
    status: 'partial',
    stage: 'partial-failure',
    current: safeTotal,
    total: safeTotal,
    message,
    repositoryName: null,
  };
}

function buildRunningTaskProgress(
  taskId: string,
  taskType: string,
  message: string,
  repositoryName: string | null = null,
  stage = 'request',
): TaskProgressEvent {
  return {
    taskId,
    taskType,
    status: 'running',
    stage,
    current: 0,
    total: 0,
    message,
    repositoryName,
  };
}

function buildSucceededTaskProgress(taskId: string, taskType: string, message: string): TaskProgressEvent {
  return {
    taskId,
    taskType,
    status: 'succeeded',
    stage: 'done',
    current: 1,
    total: 1,
    message,
    repositoryName: null,
  };
}

function getSuggestedTagColor(tagName: string) {
  const palette = ['#2563eb', '#0f766e', '#9333ea', '#c2410c', '#16a34a', '#be123c', '#0891b2', '#7c3aed'];
  const index = Array.from(tagName).reduce((sum, char) => sum + char.charCodeAt(0), 0) % palette.length;
  return palette[index];
}
