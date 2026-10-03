import ReactDOM from 'react-dom/client';
import { App } from './App';
import { AppErrorBoundary } from './components/error-boundary';
import { ToastProvider } from './components/kit';
import '@fontsource-variable/material-symbols-outlined/fill.css';
import './styles/design-system.css';
import './styles.css';
import './styles/kit.css';

// 图标字体就绪后再显示图标，免得先闪出 home / psychology 这样的英文单词（最多等 1.5 秒）
const markIconsReady = () => document.documentElement.setAttribute('data-icons-ready', '');
if (typeof document !== 'undefined' && document.fonts?.load) {
  void Promise.race([
    document.fonts.load("24px 'Material Symbols Outlined Variable'"),
    new Promise((resolve) => window.setTimeout(resolve, 1500)),
  ]).finally(markIconsReady);
} else {
  markIconsReady();
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <AppErrorBoundary>
    <ToastProvider>
      <App />
    </ToastProvider>
  </AppErrorBoundary>,
);
