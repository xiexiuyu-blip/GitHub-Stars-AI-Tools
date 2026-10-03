import { Component, type ErrorInfo, type ReactNode } from 'react';

type AppErrorBoundaryProps = { children: ReactNode };
type AppErrorBoundaryState = { error: Error | null };

/**
 * 顶层兜底：任何一个页面在渲染时崩了，不再整窗变白，而是显示具体报错，并可以复制。
 * 崩的只是界面，本地资料库不受影响。
 */
export class AppErrorBoundary extends Component<AppErrorBoundaryProps, AppErrorBoundaryState> {
  state: AppErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): AppErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('[界面崩溃]', error, info.componentStack);
  }

  render() {
    const { error } = this.state;
    if (!error) {
      return this.props.children;
    }

    const stack = (error.stack ?? '').split('\n').slice(0, 10).join('\n');
    const detail = `${error.name}: ${error.message}\n${stack}`;

    return (
      <div role="alert" className="flex h-full items-center justify-center bg-background p-6 text-foreground">
        <div className="w-full max-w-2xl rounded-lg border border-border bg-card p-6">
          <h1 className="text-lg font-semibold">界面出错了</h1>
          <p className="mt-2 text-sm text-on-surface-variant">
            只是界面崩了，你的资料库没有受影响。点「重新加载」通常就能恢复；如果反复出现，请复制下面的信息发给开发者。
          </p>
          <details className="mt-4">
            <summary className="cursor-pointer text-xs text-on-surface-variant">看报错细节（给开发者的）</summary>
            <pre className="mt-2 max-h-64 overflow-auto whitespace-pre-wrap break-words rounded-md bg-surface-container p-3 text-xs">
              {detail}
            </pre>
          </details>
          <div className="mt-4 flex gap-2">
            <button
              type="button"
              className="h-8 rounded-md bg-primary px-3 text-sm text-on-primary"
              onClick={() => window.location.reload()}
            >
              重新加载
            </button>
            <button
              type="button"
              className="h-8 rounded-md border border-border bg-card px-3 text-sm"
              onClick={() => void navigator.clipboard?.writeText(detail)}
            >
              复制错误信息
            </button>
          </div>
        </div>
      </div>
    );
  }
}
