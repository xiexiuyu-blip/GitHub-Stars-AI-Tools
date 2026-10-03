import ReactDOM from 'react-dom/client';
import { App } from './App';
import { AppErrorBoundary } from './components/error-boundary';
import '@fontsource-variable/material-symbols-outlined/fill.css';
import './styles/design-system.css';
import './styles.css';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <AppErrorBoundary>
    <App />
  </AppErrorBoundary>,
);
