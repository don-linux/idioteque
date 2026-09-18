import { Channel, invoke } from "@tauri-apps/api/core";
import { clearsRenderCrash, renderCrashedMessage } from "$lib/browser-errors";
import { toasts } from "$lib/toast.svelte";

export type BrowserSource = "bundled" | "installed";

export interface BrowserBoot {
  cef: string;
  chromium: string;
  apiVersion: number;
  source: BrowserSource;
  noSandbox: boolean;
}

export type BrowserCommand =
  | { cmd: "navigate"; url: string }
  | { cmd: "back" }
  | { cmd: "forward" }
  | { cmd: "stop" }
  | { cmd: "reload"; ignoreCache: boolean }
  | { cmd: "devtools" }
  | { cmd: "show" }
  | { cmd: "hide" };

export type BrowserEvent =
  | { event: "ready"; cef: string; chromium: string; apiVersion: number }
  | { event: "nav"; url: string; canGoBack: boolean; canGoForward: boolean; loading: boolean }
  | { event: "title"; title: string }
  | { event: "load-end"; status: number }
  | { event: "load-error"; code: number; text: string; url: string }
  | { event: "shortcut"; chord: string }
  | { event: "render-crashed"; status: string }
  | { event: "health"; ok: boolean; cef: string; chromium: string; apiVersion: number }
  | { event: "fatal"; message: string; code: number }
  | { event: "exit"; code: number };

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
    return "sin compositor Wayland";
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
  visible = $state(false);
  error = $state<string | null>(null);
  url = $state("about:blank");
  title = $state("");
  loading = $state(false);
  canGoBack = $state(false);
  canGoForward = $state(false);
  boot = $state<BrowserBoot | null>(null);
  noSandbox = $state(false);

  #gen = 0;
  /** First-attempt sandbox fatal leaked by ADE; keep booting and ignore a late copy after ready. */
  #suppressRetryFatal = false;

  async enter(): Promise<void> {
    this.started = true;
    if (this.alive) {
      await this.setVisible(true);
      return;
    }
    if (!this.booting) await this.spawn();
  }

  async toggle(): Promise<void> {
    if (this.alive && this.visible) {
      await this.setVisible(false);
      return;
    }
    await this.enter();
  }

  async spawn(): Promise<void> {
    if (this.alive || this.booting) return;

    const gen = ++this.#gen;
    this.#suppressRetryFatal = false;
    this.booting = true;
    this.error = null;
    this.started = true;

    const onEvent = new Channel<BrowserEvent>();
    onEvent.onmessage = (event) => {
      if (gen !== this.#gen) return;
      this.#onEvent(event);
    };

    try {
      const boot = await invoke<BrowserBoot>("browser_spawn", {
        url: this.url,
        onEvent,
      });
      if (gen !== this.#gen) return;
      this.boot = boot;
      this.noSandbox = boot.noSandbox;
    } catch (error) {
      if (gen !== this.#gen) return;
      this.booting = false;
      this.alive = false;
      this.visible = false;
      this.#setError(messageFrom(error));
    }
  }

  async setVisible(visible: boolean): Promise<void> {
    if (!this.alive) return;

    try {
      await invoke("browser_set_visible", { visible });
      this.visible = visible;
    } catch (error) {
      this.#setError(messageFrom(error));
    }
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

  async teardown(): Promise<void> {
    this.#gen += 1;
    this.#suppressRetryFatal = false;
    this.started = false;
    this.alive = false;
    this.booting = false;
    this.visible = false;
    this.error = null;
    this.url = "about:blank";
    this.title = "";
    this.loading = false;
    this.canGoBack = false;
    this.canGoForward = false;
    this.boot = null;
    this.noSandbox = false;

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
      this.#setError(messageFrom(error));
    }
  }

  #setError(message: string): void {
    this.error = message;
    toasts.notice(message);
  }

  #onEvent(event: BrowserEvent): void {
    if (isSandboxRetryFatal(event) && this.#holdSandboxRetryFatal()) return;

    switch (event.event) {
      case "ready":
        this.alive = true;
        this.booting = false;
        this.visible = true;
        this.error = null;
        return;
      case "nav":
        this.url = event.url;
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
        this.#setError(event.text || "No se pudo cargar la página");
        return;
      case "shortcut":
        this.#onShortcut(event.chord);
        return;
      case "render-crashed":
        this.loading = false;
        this.#setError(renderCrashedMessage(event.status));
        return;
      case "fatal":
        this.booting = false;
        this.alive = false;
        this.visible = false;
        this.#setError(event.message);
        return;
      case "exit":
        this.alive = false;
        this.booting = false;
        this.loading = false;
        this.visible = false;
        if (event.code === 1 || event.code === 11 || event.code === 15) {
          this.#suppressRetryFatal = true;
        }
        if (event.code !== 0) {
          const copy = messageForExitCode(event.code);
          if (copy) this.#setError(copy);
        }
        return;
      case "health":
        return;
    }
  }

  /**
   * CONTRACT: the first sandbox-retry fatal must not leave the window machine
   * looking dead. ADE is supposed to swallow it; if the Channel still delivers
   * it (or delivers it late after ready), keep booting and do not kill a
   * recovered session.
   */
  #holdSandboxRetryFatal(): boolean {
    if (this.booting || this.alive) {
      this.#suppressRetryFatal = true;
      return true;
    }
    return this.#suppressRetryFatal;
  }

  #onShortcut(chord: string): void {
    if (chord === "ctrl+b" || chord === "ctrl+shift+b") {
      void this.toggle();
    }
  }
}

export const browser = new BrowserState();
