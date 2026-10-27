import { create } from 'zustand';

export type ToastTone = 'success' | 'error' | 'info' | 'warning';

export interface ToastItem {
  id: string;
  tone: ToastTone;
  title: string;
  description?: string;
}

interface ToastStore {
  items: ToastItem[];
  push: (tone: ToastTone, title: string, description?: string) => string;
  dismiss: (id: string) => void;
  success: (title: string, description?: string) => string;
  error: (title: string, description?: string) => string;
  info: (title: string, description?: string) => string;
  warning: (title: string, description?: string) => string;
}

let counter = 0;

function createId(): string {
  counter += 1;
  return `toast-${Date.now()}-${counter}`;
}

export const useToastStore = create<ToastStore>((set, get) => ({
  items: [],
  push: (tone, title, description) => {
    const id = createId();
    set((state) => ({ items: [...state.items, { id, tone, title, description }].slice(-5) }));
    window.setTimeout(() => get().dismiss(id), tone === 'error' ? 6000 : 3800);
    return id;
  },
  dismiss: (id) => set((state) => ({ items: state.items.filter((item) => item.id !== id) })),
  success: (title, description) => get().push('success', title, description),
  error: (title, description) => get().push('error', title, description),
  info: (title, description) => get().push('info', title, description),
  warning: (title, description) => get().push('warning', title, description),
}));

export function toastError(error: unknown): void {
  const message =
    error instanceof Error
      ? error.message
      : typeof error === 'string'
        ? error
        : '操作失败，请查看日志';
  useToastStore.getState().error('操作失败', message);
}
