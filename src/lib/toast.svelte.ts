import {
  TOAST_DURATION_MS,
  createSuccessToast,
  withoutToast,
  type ToastItem,
} from "$lib/toast";

class ToastBus {
  items = $state<ToastItem[]>([]);
  #nextId = 0;
  #timers = new Map<number, ReturnType<typeof setTimeout>>();

  success(message: string): number {
    const id = ++this.#nextId;
    this.items = [...this.items, createSuccessToast(id, message)];
    const timer = setTimeout(() => this.dismiss(id), TOAST_DURATION_MS);
    this.#timers.set(id, timer);
    return id;
  }

  dismiss(id: number): void {
    const timer = this.#timers.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      this.#timers.delete(id);
    }
    this.items = withoutToast(this.items, id);
  }
}

export const toasts = new ToastBus();
