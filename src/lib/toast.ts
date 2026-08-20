export const TOAST_DURATION_MS = 3000;
export const SETTINGS_SAVED_TOAST = "Configuración guardada";

export interface ToastItem {
  id: number;
  message: string;
  type: "success";
}

export function createSuccessToast(id: number, message: string): ToastItem {
  return { id, message, type: "success" };
}

export function withoutToast(items: readonly ToastItem[], id: number): ToastItem[] {
  return items.filter((item) => item.id !== id);
}
