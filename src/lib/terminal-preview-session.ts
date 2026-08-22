import { Channel, invoke } from "@tauri-apps/api/core";
import { PREVIEW_PTY_ID } from "$lib/pty";

export { PREVIEW_PTY_ID };

function messageFrom(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

export class TerminalPreviewSession {
  alive = false;
  error: string | null = null;

  #spawning = false;
  #gen = 0;
  #onData: ((chunk: string) => void) | null = null;

  attachWriter(write: (chunk: string) => void): void {
    this.#onData = write;
  }

  detachWriter(): void {
    this.#onData = null;
  }

  async spawn(cols: number, rows: number): Promise<void> {
    if (this.alive || this.#spawning) return;

    this.#spawning = true;
    this.error = null;
    const gen = ++this.#gen;

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
        id: PREVIEW_PTY_ID,
        cwd: "",
        cols,
        rows,
        onData,
        onExit,
      });

      if (gen !== this.#gen) {
        try {
          await invoke("pty_kill", { id: PREVIEW_PTY_ID });
        } catch {
          // The session may already be gone.
        }
        return;
      }

      this.alive = true;
    } catch (error) {
      this.alive = false;
      this.error = messageFrom(error);
    } finally {
      this.#spawning = false;
    }
  }

  async resize(cols: number, rows: number): Promise<void> {
    if (!this.alive) return;

    try {
      await invoke("pty_resize", { id: PREVIEW_PTY_ID, cols, rows });
    } catch (error) {
      this.error = messageFrom(error);
    }
  }

  async kill(): Promise<void> {
    this.#gen += 1;
    this.alive = false;
    this.#spawning = false;

    try {
      await invoke("pty_kill", { id: PREVIEW_PTY_ID });
    } catch {
      // The session may already be gone.
    }
  }
}
