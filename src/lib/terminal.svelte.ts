import { Channel, invoke } from "@tauri-apps/api/core";

export type TerminalDock = "bottom" | "right";

const DEFAULT_BOTTOM = 280;
const DEFAULT_RIGHT = 380;
const MIN_BOTTOM = 120;
const MIN_RIGHT = 200;

function messageFrom(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

class TerminalPanelState {
  open = $state(false);
  dock = $state<TerminalDock>("bottom");
  bottomSize = $state(DEFAULT_BOTTOM);
  rightSize = $state(DEFAULT_RIGHT);
  alive = $state(false);
  error = $state<string | null>(null);

  #spawning = false;
  #onData: ((chunk: string) => void) | null = null;

  get size(): number {
    return this.dock === "bottom" ? this.bottomSize : this.rightSize;
  }

  toggle(dock: TerminalDock): void {
    if (!this.open) {
      this.dock = dock;
      this.open = true;
      this.error = null;
      return;
    }

    if (this.dock === dock) {
      this.open = false;
      return;
    }

    this.dock = dock;
  }

  setSize(pixels: number, viewport: number): void {
    const minimum = this.dock === "bottom" ? MIN_BOTTOM : MIN_RIGHT;
    const maximum = Math.max(minimum, Math.floor(viewport * 0.8));
    const next = Math.min(maximum, Math.max(minimum, Math.round(pixels)));

    if (this.dock === "bottom") {
      this.bottomSize = next;
    } else {
      this.rightSize = next;
    }
  }

  attachWriter(write: (chunk: string) => void): void {
    this.#onData = write;
  }

  detachWriter(): void {
    this.#onData = null;
  }

  async spawn(cwd: string, cols: number, rows: number): Promise<void> {
    if (this.alive || this.#spawning) return;

    this.#spawning = true;
    this.error = null;

    const onData = new Channel<string>();
    onData.onmessage = (chunk) => {
      this.#onData?.(chunk);
    };

    const onExit = new Channel<number>();
    onExit.onmessage = () => {
      this.alive = false;
    };

    try {
      await invoke("pty_spawn", {
        cwd,
        cols,
        rows,
        onData,
        onExit,
      });
      this.alive = true;
    } catch (error) {
      this.alive = false;
      this.error = messageFrom(error);
    } finally {
      this.#spawning = false;
    }
  }

  async write(data: string): Promise<void> {
    if (!this.alive) return;

    try {
      await invoke("pty_write", { data });
    } catch (error) {
      this.error = messageFrom(error);
    }
  }

  async resize(cols: number, rows: number): Promise<void> {
    if (!this.alive) return;

    try {
      await invoke("pty_resize", { cols, rows });
    } catch (error) {
      this.error = messageFrom(error);
    }
  }

  async teardown(): Promise<void> {
    this.open = false;
    this.alive = false;
    this.error = null;
    this.#spawning = false;

    try {
      await invoke("pty_kill");
    } catch {
      // The session may already be gone.
    }
  }
}

export const terminal = new TerminalPanelState();
