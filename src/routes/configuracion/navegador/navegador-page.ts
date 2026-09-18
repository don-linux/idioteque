import {
  CEF_CHECKING,
  CEF_UNAVAILABLE,
  formatCheckedAt,
  formatDenyEntry,
  formatPendingPromotion,
  sourceLabel,
  type CefRuntimeInfo,
  type DenyEntry,
  type PendingPromotion,
  type SlotInfo,
} from "$lib/cef-runtime";

export { CEF_CHECKING, CEF_UNAVAILABLE };

export const CEF_CYCLE_RUNNING = "Ya hay un ciclo del updater CEF en curso";
export const DENYLIST_NONE = "ninguna";
export const CHECK_DISABLE_MS = 3000;
export const RELOAD_AFTER_CHECK_MS = 5000;

export const COPY = {
  heading: "Navegador",
  lead: "Motor Chromium embebido. No hay nada que guardar aquí: es información y un botón.",
  loading: "Cargando…",
  check: "Buscar actualización",
  denylistLabel: "Versiones descartadas:",
} as const;

export type NavegadorPageSnapshot = {
  info: CefRuntimeInfo | null;
  error: string | null;
  status: string | null;
  checking: boolean;
};

export type RuntimeLines = {
  chromium: string;
  cef: string;
  base: string;
  lastCheck: string;
  pending: string | null;
  denylistLabel: string;
  denylist: string[];
};

export function emptySnapshot(): NavegadorPageSnapshot {
  return { info: null, error: null, status: null, checking: false };
}

function text(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function isSlot(value: unknown): value is SlotInfo {
  if (value === null || typeof value !== "object") return false;
  const slot = value as Partial<SlotInfo>;
  return Boolean(text(slot.chromiumVersion) && text(slot.cefVersion));
}

function isDenyEntry(value: unknown): value is DenyEntry {
  if (value === null || typeof value !== "object") return false;
  const entry = value as Partial<DenyEntry>;
  return Boolean(text(entry.chromiumVersion) && text(entry.reason));
}

function pendingPromotion(value: unknown): PendingPromotion | null {
  if (value === null || typeof value !== "object") return null;
  const pending = value as Partial<PendingPromotion>;
  const chromiumVersion = text(pending.chromiumVersion);
  if (!chromiumVersion) return null;
  return {
    cefVersion: text(pending.cefVersion),
    chromiumVersion,
  };
}

/** Missing / partial `cef_runtime_info` must not reach the template. */
export function acceptRuntimeInfo(value: unknown): CefRuntimeInfo | null {
  if (value === null || typeof value !== "object") return null;
  const raw = value as Partial<CefRuntimeInfo>;
  if (!isSlot(raw.current) || !isSlot(raw.base)) return null;

  return {
    current: raw.current,
    base: raw.base,
    candidate: isSlot(raw.candidate) ? raw.candidate : null,
    denylist: Array.isArray(raw.denylist) ? raw.denylist.filter(isDenyEntry) : [],
    lastCheckAt: typeof raw.lastCheckAt === "string" ? raw.lastCheckAt : null,
    pendingPromotion: pendingPromotion(raw.pendingPromotion),
    hostApiVersion: typeof raw.hostApiVersion === "number" ? raw.hostApiVersion : 0,
    platform: typeof raw.platform === "string" ? raw.platform : "",
    hostAlive: raw.hostAlive === true,
  };
}

function isIpcNoise(message: string): boolean {
  const lower = message.toLowerCase();
  if (lower.includes("command") && lower.includes("not found")) return true;
  if (lower.includes("not available")) return true;
  if (lower.includes("invoke") && (lower.includes("fail") || lower.includes("not"))) {
    return true;
  }
  if (lower.includes("ipc") && (lower.includes("fail") || lower.includes("not"))) {
    return true;
  }
  return false;
}

function extractMessage(caught: unknown): string {
  if (typeof caught === "string") return caught.trim();
  if (caught instanceof Error) return caught.message.trim();
  if (caught !== null && typeof caught === "object") {
    const record = caught as { message?: unknown; error?: unknown };
    if (typeof record.message === "string") return record.message.trim();
    if (typeof record.error === "string") return record.error.trim();
  }
  return "";
}

/** Check-update errors: keep the Spanish cycle copy, never IPC English. */
export function messageFrom(caught: unknown): string {
  const raw = extractMessage(caught);
  if (!raw || isIpcNoise(raw)) return CEF_UNAVAILABLE;
  if (raw === CEF_CYCLE_RUNNING) return CEF_CYCLE_RUNNING;
  return raw;
}

export function runtimeLines(info: CefRuntimeInfo): RuntimeLines {
  return {
    chromium: `Chromium actual: ${info.current.chromiumVersion} (${sourceLabel(info.current.source)})`,
    cef: `CEF: ${info.current.cefVersion}`,
    base: `Base de fábrica: Chromium ${info.base.chromiumVersion}`,
    lastCheck: `Última comprobación: ${formatCheckedAt(info.lastCheckAt)}`,
    pending: info.pendingPromotion
      ? formatPendingPromotion(info.pendingPromotion)
      : null,
    denylistLabel: COPY.denylistLabel,
    denylist: info.denylist.map(formatDenyEntry),
  };
}

export function createNavegadorPage(opts: {
  invoke: (command: string) => Promise<unknown>;
  onChange?: (snapshot: NavegadorPageSnapshot) => void;
}): {
  load: () => Promise<void>;
  checkUpdates: () => Promise<void>;
  dispose: () => void;
  snapshot: () => NavegadorPageSnapshot;
} {
  const state = emptySnapshot();
  let disposed = false;
  let disableTimer: ReturnType<typeof setTimeout> | undefined;
  let reloadTimer: ReturnType<typeof setTimeout> | undefined;

  function emit(): void {
    opts.onChange?.({
      info: state.info,
      error: state.error,
      status: state.status,
      checking: state.checking,
    });
  }

  function clearTimers(): void {
    if (disableTimer !== undefined) clearTimeout(disableTimer);
    if (reloadTimer !== undefined) clearTimeout(reloadTimer);
    disableTimer = undefined;
    reloadTimer = undefined;
  }

  async function load(): Promise<void> {
    try {
      const next = acceptRuntimeInfo(await opts.invoke("cef_runtime_info"));
      if (disposed) return;
      if (!next) {
        state.info = null;
        state.error = CEF_UNAVAILABLE;
      } else {
        state.info = next;
        state.error = null;
      }
      emit();
    } catch {
      if (disposed) return;
      state.info = null;
      state.error = CEF_UNAVAILABLE;
      emit();
    }
  }

  async function checkUpdates(): Promise<void> {
    if (disposed || state.checking) return;

    clearTimers();
    state.checking = true;
    state.status = CEF_CHECKING;
    emit();

    try {
      await opts.invoke("cef_check_updates");
    } catch (caught) {
      if (!disposed) {
        state.status = messageFrom(caught);
        emit();
      }
    }

    if (disposed) return;

    disableTimer = setTimeout(() => {
      if (disposed) return;
      state.checking = false;
      emit();
    }, CHECK_DISABLE_MS);

    reloadTimer = setTimeout(() => {
      void load().then(() => {
        if (disposed) return;
        if (state.status === CEF_CHECKING) {
          state.status = null;
          emit();
        }
      });
    }, RELOAD_AFTER_CHECK_MS);
  }

  function dispose(): void {
    disposed = true;
    clearTimers();
  }

  return {
    load,
    checkUpdates,
    dispose,
    snapshot: () => ({
      info: state.info,
      error: state.error,
      status: state.status,
      checking: state.checking,
    }),
  };
}
