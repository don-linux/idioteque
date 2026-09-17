import { Channel, invoke } from "@tauri-apps/api/core";
import { clearsRenderCrash, renderCrashedMessage } from "$lib/browser-errors";
import { folderVisibility } from "$lib/folder-visibility.svelte";
import { displayUrl, normalizeUrlInput } from "$lib/browser-url";
import { unsavedExit } from "$lib/unsaved-exit.svelte";
import { surface } from "$lib/workspace-surface.svelte";

export type BrowserSource = "bundled" | "installed";

export interface BrowserBoot {
  cef: string;
  chromium: string;
  apiVersion: number;
  source: BrowserSource;
  noSandbox: boolean;
}

export interface BrowserBounds {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type BrowserCommand =
  | { cmd: "navigate"; url: string }
  | { cmd: "back" }
  | { cmd: "forward" }
  | { cmd: "stop" }
  | { cmd: "reload"; ignoreCache: boolean }
  | { cmd: "focus" }
  | { cmd: "devtools" };

export type FocusOwner = "app" | "browser";

export type BrowserEvent =
  | { event: "ready"; cef: string; chromium: string; apiVersion: number; xid: number }
  | { event: "nav"; url: string; canGoBack: boolean; canGoForward: boolean; loading: boolean }
  | { event: "title"; title: string }
  | { event: "load-end"; status: number }
  | { event: "load-error"; code: number; text: string; url: string }
  | { event: "shortcut"; chord: string }
  | { event: "focus"; owner: FocusOwner; next?: boolean }
  | { event: "render-crashed"; status: string }
  | { event: "health"; ok: boolean; cef: string; chromium: string; apiVersion: number }
  | { event: "fatal"; message: string; code: number }
  | { event: "exit"; code: number };

/** Toolbar / app input that must not keep keys while CEF owns the keyboard. */
export type KeyboardTarget = {
  tagName?: string;
  isContentEditable?: boolean;
  closest?: (selector: string) => unknown;
  blur?: () => void;
  focus?: () => void;
};

export function isAppKeyboardTarget(el: KeyboardTarget | null | undefined): boolean {
  if (!el) return false;
  if (typeof el.closest === "function" && el.closest("[data-browser-toolbar]")) return true;
  const tag = el.tagName?.toLowerCase();
  if (tag === "input" || tag === "textarea" || tag === "select") return true;
  return el.isContentEditable === true;
}

/** One `browser_focus_app` per chrome activation; skip if we already own keys. */
export function shouldClaimAppFocus(owner: FocusOwner): boolean {
  return owner !== "app";
}

/** Show-time rAF must not steal keys from the URL bar. */
export function shouldGiftCefFocus(owner: FocusOwner): boolean {
  return owner !== "app";
}

export function resolveChromeTarget(
  toolbar: { querySelector?: (selector: string) => KeyboardTarget | null } | null | undefined,
  next: boolean,
): KeyboardTarget | null {
  if (!toolbar?.querySelector) return null;
  return toolbar.querySelector(next ? "[data-browser-url]" : "[data-browser-chrome-last]");
}

function pageDocument(): {
  activeElement?: KeyboardTarget | null;
  querySelector?: (
    selector: string,
  ) => { querySelector?: (selector: string) => KeyboardTarget | null } | null;
} | null {
  if (typeof document === "undefined") return null;
  return document;
}

function blurActiveAppKeyboard(): void {
  const active = pageDocument()?.activeElement ?? null;
  if (!isAppKeyboardTarget(active)) return;
  active.blur?.();
}

function focusToolbarChrome(next: boolean): void {
  const toolbar = pageDocument()?.querySelector?.("[data-browser-toolbar]") ?? null;
  resolveChromeTarget(toolbar, next)?.focus?.();
}

function messageFrom(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

function messageForExitCode(code: number): string | null {
  if (code === 0) return null;
  if (code === 10 || code === 13 || code === 14) {
    return "El motor Chromium no es compatible con esta versión de idioteque";
  }
  if (code === 15) {
    return "El sandbox de Chromium no está disponible";
  }
  if (code === 16) {
    return "El navegador necesita X11";
  }
  return "El navegador se cerró inesperadamente";
}

/** ADE retries once on exit 15 / abort 1 / initialize 11 (CONTRACT 4.1 / 5). */
function isSandboxRetryFatal(event: BrowserEvent): event is Extract<BrowserEvent, { event: "fatal" }> {
  return event.event === "fatal" && (event.code === 1 || event.code === 11 || event.code === 15);
}

class BrowserState {
  started = $state(false);
  alive = $state(false);
  booting = $state(false);
  error = $state<string | null>(null);
  url = $state("about:blank");
  inputUrl = $state("");
  title = $state("");
  loading = $state(false);
  canGoBack = $state(false);
  canGoForward = $state(false);
  boot = $state<BrowserBoot | null>(null);
  noSandbox = $state(false);
  focusUrlRequested = $state(0);
  pendingSpawn = $state(false);
  /** Single keyboard owner: wry chrome (`app`) or the CEF child (`browser`). */
  focusOwner = $state<FocusOwner>("browser");

  visible = $derived(
    surface.current === "browser" && !unsavedExit.open && !folderVisibility.open,
  );

  #gen = 0;
  /** First-attempt sandbox fatal leaked by ADE; keep “Arrancando…” and ignore a late copy after ready. */
  #suppressRetryFatal = false;

  enter(): void {
    surface.enterBrowser();
    this.started = true;
    if (!this.alive && !this.booting) this.pendingSpawn = true;
  }

  leave(): void {
    surface.leaveBrowser();
  }

  toggle(): void {
    if (surface.current === "browser") {
      this.leave();
      return;
    }
    this.enter();
  }

  async spawn(bounds: BrowserBounds, scale: number): Promise<void> {
    if (this.alive || this.booting) return;

    const gen = ++this.#gen;
    this.#suppressRetryFatal = false;
    this.pendingSpawn = false;
    this.booting = true;
    this.error = null;

    const onEvent = new Channel<BrowserEvent>();
    onEvent.onmessage = (event) => {
      if (gen !== this.#gen) return;
      this.#onEvent(event);
    };

    try {
      const boot = await invoke<BrowserBoot>("browser_spawn", {
        url: this.url,
        bounds,
        scale,
        onEvent,
      });
      if (gen !== this.#gen) return;
      this.boot = boot;
      this.noSandbox = boot.noSandbox;
    } catch (error) {
      if (gen !== this.#gen) return;
      this.booting = false;
      this.alive = false;
      if (this.error === null) this.error = messageFrom(error);
    }
  }

  async respawn(): Promise<void> {
    await this.#shutdown({ restoreSurface: false });
    this.started = true;
    this.pendingSpawn = true;
    if (surface.current !== "browser") surface.enterBrowser();
  }

  async navigate(input: string): Promise<void> {
    const url = normalizeUrlInput(input);
    if (url === null) {
      if (input.trim().length > 0) this.error = "Introduce una URL válida";
      return;
    }

    this.url = url;
    this.inputUrl = displayUrl(url);
    this.error = null;
    await this.#command({ cmd: "navigate", url });
  }

  async back(): Promise<void> {
    if (!this.canGoBack) return;
    await this.#command({ cmd: "back" });
  }

  async forward(): Promise<void> {
    if (!this.canGoForward) return;
    await this.#command({ cmd: "forward" });
  }

  async reload(ignoreCache = false): Promise<void> {
    await this.#command({ cmd: "reload", ignoreCache });
  }

  async stop(): Promise<void> {
    await this.#command({ cmd: "stop" });
  }

  async devtools(): Promise<void> {
    await this.#command({ cmd: "devtools" });
  }

  async setBounds(rect: BrowserBounds, scale: number): Promise<void> {
    if (!this.alive) return;

    try {
      await invoke("browser_set_bounds", {
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: rect.h,
        scale,
      });
    } catch (error) {
      this.error = messageFrom(error);
    }
  }

  async setVisible(visible: boolean): Promise<void> {
    if (!this.alive) return;

    try {
      await invoke("browser_set_visible", { visible });
    } catch (error) {
      this.error = messageFrom(error);
    }
  }

  async focus(): Promise<void> {
    await this.#command({ cmd: "focus" });
  }

  /** Devuelve el foco X11 a la ventana de idioteque (la barra Svelte, el editor). */
  async focusApp(): Promise<void> {
    this.focusOwner = "app";
    try {
      await invoke("browser_focus_app");
    } catch {
      // Fuera de Tauri o sin X11: no hay foco que devolver.
    }
  }

  async teardown(): Promise<void> {
    await this.#shutdown({ restoreSurface: true });
  }

  async #shutdown(options: { restoreSurface: boolean }): Promise<void> {
    this.#gen += 1;
    this.#suppressRetryFatal = false;
    this.pendingSpawn = false;
    this.started = false;
    this.alive = false;
    this.booting = false;
    this.error = null;
    this.url = "about:blank";
    this.inputUrl = "";
    this.title = "";
    this.loading = false;
    this.canGoBack = false;
    this.canGoForward = false;
    this.boot = null;
    this.noSandbox = false;
    this.focusOwner = "browser";

    if (options.restoreSurface && surface.current === "browser") {
      surface.set("editor");
    }

    try {
      await invoke("browser_kill");
    } catch {
      // The host may already be gone.
    }
  }

  async #command(cmd: BrowserCommand): Promise<void> {
    if (!this.alive) return;

    try {
      await invoke("browser_command", { cmd });
    } catch (error) {
      this.error = messageFrom(error);
    }
  }

  #onEvent(event: BrowserEvent): void {
    if (isSandboxRetryFatal(event) && this.#holdSandboxRetryFatal()) return;

    switch (event.event) {
      case "ready":
        this.alive = true;
        this.booting = false;
        this.error = null;
        return;
      case "nav":
        this.url = event.url;
        this.inputUrl = displayUrl(event.url);
        this.canGoBack = event.canGoBack;
        this.canGoForward = event.canGoForward;
        this.loading = event.loading;
        return;
      case "title":
        this.title = event.title;
        return;
      case "load-end":
        this.loading = false;
        if (clearsRenderCrash(this.error)) this.error = null;
        return;
      case "load-error":
        if (event.code === -3) return;
        this.loading = false;
        this.error = event.text || "No se pudo cargar la página";
        return;
      case "shortcut":
        this.#onShortcut(event.chord);
        return;
      case "focus":
        this.#onFocus(event);
        return;
      case "render-crashed":
        this.loading = false;
        this.error = renderCrashedMessage(event.status);
        return;
      case "fatal":
        this.booting = false;
        this.alive = false;
        this.error = event.message;
        return;
      case "exit":
        this.alive = false;
        this.booting = false;
        this.loading = false;
        if (event.code === 1 || event.code === 11 || event.code === 15) {
          this.#suppressRetryFatal = true;
        }
        if (event.code !== 0) this.error = messageForExitCode(event.code);
        return;
      case "health":
        return;
    }
  }

  /**
   * CONTRACT: the first sandbox-retry fatal must not leave “Arrancando Chromium…”.
   * ADE is supposed to swallow it; if the Channel still delivers it (or delivers
   * it late after ready), keep booting and do not kill a recovered session.
   */
  #holdSandboxRetryFatal(): boolean {
    if (this.booting || this.alive) {
      this.#suppressRetryFatal = true;
      return true;
    }
    return this.#suppressRetryFatal;
  }

  #onFocus(event: Extract<BrowserEvent, { event: "focus" }>): void {
    if (event.owner === "browser") {
      this.focusOwner = "browser";
      blurActiveAppKeyboard();
      return;
    }
    this.focusOwner = "app";
    void this.focusApp();
    focusToolbarChrome(event.next !== false);
  }

  #onShortcut(chord: string): void {
    if (chord === "ctrl+b" || chord === "ctrl+shift+b") {
      this.leave();
      void this.focusApp();
      return;
    }
    if (chord === "ctrl+l") {
      this.focusUrlRequested += 1;
    }
  }
}

export const browser = new BrowserState();
