import { createContext, ReactNode, useCallback, useContext, useEffect, useRef, useState } from 'react';
import { Icon } from '@/components/ui/icon';
import { Button } from './button';
import {
  pushToast,
  removeToast,
  toastDuration,
  type ToastInput,
  type ToastRecord,
} from './toast-state';

type ToastApi = {
  show: (input: ToastInput) => void;
};

const ToastContext = createContext<ToastApi | null>(null);

export function useToast(): ToastApi {
  const api = useContext(ToastContext);
  if (!api) throw new Error('提示条还没挂上');
  return api;
}

export function ToastProvider(props: { children: ReactNode }) {
  const [items, setItems] = useState<ToastRecord[]>([]);
  const itemsRef = useRef<ToastRecord[]>([]);
  const timersRef = useRef(new Map<string, number>());

  const dismiss = useCallback((id: string) => {
    const timer = timersRef.current.get(id);
    if (timer != null) window.clearTimeout(timer);
    timersRef.current.delete(id);
    const next = removeToast(itemsRef.current, id);
    itemsRef.current = next;
    setItems(next);
  }, []);

  const show = useCallback((input: ToastInput) => {
    const id = `toast-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    const record: ToastRecord = { ...input, id };
    const next = pushToast(itemsRef.current, record);
    const keep = new Set(next.map((item) => item.id));
    for (const item of itemsRef.current) {
      if (keep.has(item.id)) continue;
      const timer = timersRef.current.get(item.id);
      if (timer != null) window.clearTimeout(timer);
      timersRef.current.delete(item.id);
    }
    itemsRef.current = next;
    setItems(next);

    const duration = toastDuration(input);
    if (duration > 0) {
      const timer = window.setTimeout(() => dismiss(id), duration);
      timersRef.current.set(id, timer);
    }
  }, [dismiss]);

  useEffect(() => {
    const timers = timersRef.current;
    return () => {
      for (const timer of timers.values()) window.clearTimeout(timer);
      timers.clear();
    };
  }, []);

  return (
    <ToastContext.Provider value={{ show }}>
      {props.children}
      <div className="pointer-events-none fixed bottom-6 left-1/2 z-[70] flex w-max max-w-[calc(100vw-2rem)] -translate-x-1/2 flex-col items-center gap-2">
        {items.map((item) => (
          <div
            key={item.id}
            role="status"
            className="pointer-events-auto flex max-w-full items-center gap-2.5 rounded-lg bg-card py-2.5 pr-2.5 pl-3.5 text-sm text-foreground shadow-lg"
          >
            <Icon name="check_circle" size={18} className="shrink-0 text-accent-foreground" />
            <div className="min-w-0">
              {item.title}
              {item.sub ? <span className="ml-1.5 text-xs text-on-surface-variant">{item.sub}</span> : null}
            </div>
            {item.actions?.map((action) => (
              <Button
                key={action.label}
                variant="text"
                size="sm"
                onClick={() => {
                  action.onClick();
                  dismiss(item.id);
                }}
              >
                {action.label}
              </Button>
            ))}
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}
