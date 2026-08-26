import { Channel, invoke } from "@tauri-apps/api/core";
import { nextActiveAfterClose } from "$lib/editor-tabs";
import { MAX_TERMINAL_SESSIONS, workspacePtyId } from "$lib/pty";
import { nextDockToggle, type TerminalDock } from "$lib/terminal-dock";

export type { TerminalDock };
export { MAX_TERMINAL_SESSIONS };

export type WorkspaceSurface = "editor" | "terminals";

export interface TerminalSession {
  id: string;
  alive: boolean;
  error: string | null;
}

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
  surface = $state<WorkspaceSurface>("editor");
  open = $state(false);
  started = $state(false);
  dock = $state<TerminalDock>("bottom");
  bottomSize = $state(DEFAULT_BOTTOM);
  rightSize = $state(DEFAULT_RIGHT);
  parkWidth = $state(640);
  parkHeight = $state(DEFAULT_BOTTOM);
  sessions = $state<TerminalSession[]>([]);
  activeId = $state<string | null>(null);

  #nextSerial = 1;
  #spawning = new Set<string>();
  #writers = new Map<string, (chunk: string) => void>();

  get size(): number {
    return this.dock === "bottom" ? this.bottomSize : this.rightSize;
  }

  get canAdd(): boolean {
    return this.sessions.length < MAX_TERMINAL_SESSIONS;
  }

  get error(): string | null {
    return this.sessions.find((session) => session.error)?.error ?? null;
  }

  get peeking(): boolean {
    return this.surface === "editor" && this.open;
  }

  session(id: string): TerminalSession | undefined {
    return this.sessions.find((session) => session.id === id);
  }

  isVisible(id: string): boolean {
    if (this.surface === "terminals") return true;
    if (!this.open) return false;
    return this.activeId === id;
  }

  toggle(dock: TerminalDock): void {
    if (this.surface === "terminals") return;

    const next = nextDockToggle(this.open, this.dock, dock);
    this.open = next.open;
    this.dock = next.dock;

    if (next.open) {
      this.started = true;
      this.ensureSession();
    }
  }

  rememberPark(width: number, height: number): void {
    if (width >= 2) this.parkWidth = width;
    if (height >= 2) this.parkHeight = height;
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

  ensureSession(): string | null {
    if (this.activeId && this.session(this.activeId)) return this.activeId;
    if (this.sessions.length > 0) {
      this.activeId = this.sessions[0].id;
      return this.activeId;
    }
    return this.addSession();
  }

  addSession(): string | null {
    if (!this.canAdd) return null;

    const id = workspacePtyId(this.#nextSerial);
    this.#nextSerial += 1;
    this.sessions = [...this.sessions, { id, alive: false, error: null }];
    this.activeId = id;
    this.started = true;
    return id;
  }

  focus(id: string): void {
    if (this.session(id)) this.activeId = id;
  }

  enterTerminals(): void {
    this.surface = "terminals";
    this.started = true;
    this.ensureSession();
  }

  leaveTerminals(): void {
    this.surface = "editor";
    this.open = false;
  }

  attachWriter(id: string, write: (chunk: string) => void): void {
    this.#writers.set(id, write);
  }

  detachWriter(id: string): void {
    this.#writers.delete(id);
  }

  async spawn(id: string, cwd: string, cols: number, rows: number): Promise<void> {
    const session = this.session(id);
    if (!session || session.alive || this.#spawning.has(id)) return;

    this.#spawning.add(id);
    this.#patch(id, { error: null });

    const onData = new Channel<string>();
    onData.onmessage = (chunk) => {
      this.#writers.get(id)?.(chunk);
    };

    const onExit = new Channel<number>();
    onExit.onmessage = () => {
      this.#patch(id, { alive: false });
    };

    try {
      await invoke("pty_spawn", {
        id,
        cwd,
        cols,
        rows,
        onData,
        onExit,
      });
      this.#patch(id, { alive: true });
    } catch (error) {
      this.#patch(id, { alive: false, error: messageFrom(error) });
    } finally {
      this.#spawning.delete(id);
    }
  }

  async write(id: string, data: string): Promise<void> {
    if (!this.session(id)?.alive) return;

    try {
      await invoke("pty_write", { id, data });
    } catch (error) {
      this.#patch(id, { error: messageFrom(error) });
    }
  }

  async resize(id: string, cols: number, rows: number): Promise<void> {
    if (!this.session(id)?.alive) return;

    try {
      await invoke("pty_resize", { id, cols, rows });
    } catch (error) {
      this.#patch(id, { error: messageFrom(error) });
    }
  }

  async closeSession(id: string): Promise<void> {
    if (!this.session(id)) return;

    const next = nextActiveAfterClose(
      this.sessions.map((session) => session.id),
      id,
      this.activeId,
    );

    this.#spawning.delete(id);
    this.#writers.delete(id);
    this.sessions = this.sessions.filter((session) => session.id !== id);
    this.activeId = next;

    try {
      await invoke("pty_kill", { id });
    } catch {
      // The session may already be gone.
    }

    if (this.sessions.length > 0) return;

    this.activeId = null;
    if (this.surface === "editor") {
      this.open = false;
      this.started = false;
    }
  }

  async teardown(): Promise<void> {
    this.surface = "editor";
    this.open = false;
    this.started = false;
    this.sessions = [];
    this.activeId = null;
    this.dock = "bottom";
    this.#nextSerial = 1;
    this.#spawning.clear();
    this.#writers.clear();

    try {
      await invoke("pty_kill_all");
    } catch {
      // Sessions may already be gone.
    }
  }

  #patch(id: string, patch: Partial<TerminalSession>): void {
    this.sessions = this.sessions.map((session) =>
      session.id === id ? { ...session, ...patch } : session,
    );
  }
}

export const terminal = new TerminalPanelState();
