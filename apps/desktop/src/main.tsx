import ReactDOM from 'react-dom/client';
import { App } from './App';
import { AppErrorBoundary } from './components/error-boundary';
import { ToastProvider } from './components/kit';
import '@fontsource-variable/material-symbols-outlined/fill.css';
import './styles/design-system.css';
import './styles.css';
import './styles/kit.css';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <AppErrorBoundary>
    <ToastProvider>
      <App />
    </ToastProvider>
  </AppErrorBoundary>,
);
