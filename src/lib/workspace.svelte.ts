import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

const AUTOSAVE_DELAY_MS = 500;

export type NodeKind = "dir" | "file";

export interface TreeNode {
  name: string;
  path: string;
  kind: NodeKind;
  children: TreeNode[];
}

export type SaveState = "idle" | "saving" | "saved" | "error";

interface PendingWrite {
  path: string;
  contents: string;
}

function messageFrom(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

class Workspace {
  root = $state<string | null>(null);
  tree = $state<TreeNode[]>([]);
  currentPath = $state<string | null>(null);
  content = $state("");
  dirty = $state(false);
  saveState = $state<SaveState>("idle");
  error = $state<string | null>(null);

  #timer: ReturnType<typeof setTimeout> | null = null;
  #pending: PendingWrite | null = null;
  #writing: Promise<void> = Promise.resolve();
  /// Guards against a slow read landing after the user picked another file.
  #loadToken = 0;

  get hasMarkdown(): boolean {
    return this.tree.length > 0;
  }

  async openFolder(): Promise<void> {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return;

    await this.flushSave();

    this.root = selected;
    this.currentPath = null;
    this.content = "";
    this.dirty = false;
    this.saveState = "idle";
    this.error = null;
    this.#loadToken += 1;

    await this.refreshTree();
  }

  async refreshTree(): Promise<void> {
    const root = this.root;
    if (!root) return;

    try {
      this.tree = await invoke<TreeNode[]>("list_context_tree", { root });
    } catch (error) {
      this.tree = [];
      this.error = messageFrom(error);
    }
  }

  async openFile(path: string): Promise<void> {
    const root = this.root;
    if (!root || path === this.currentPath) return;

    await this.flushSave();

    const token = ++this.#loadToken;
    this.currentPath = path;
    this.dirty = false;
    this.saveState = "idle";
    this.error = null;

    try {
      const contents = await invoke<string>("read_markdown", { root, path });
      if (token !== this.#loadToken) return;
      this.content = contents;
    } catch (error) {
      if (token !== this.#loadToken) return;
      this.content = "";
      this.error = messageFrom(error);
    }
  }

  edit(contents: string): void {
    const path = this.currentPath;
    if (!path) return;

    this.content = contents;
    this.dirty = true;
    this.#pending = { path, contents };

    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = setTimeout(() => {
      this.#timer = null;
      void this.#drainPending();
    }, AUTOSAVE_DELAY_MS);
  }

  async flushSave(): Promise<void> {
    if (this.#timer !== null) {
      clearTimeout(this.#timer);
      this.#timer = null;
    }

    await this.#drainPending();
  }

  #drainPending(): Promise<void> {
    const pending = this.#pending;
    if (!pending) return this.#writing;

    this.#pending = null;
    this.#writing = this.#writing.then(() => this.#write(pending));
    return this.#writing;
  }

  async #write(pending: PendingWrite): Promise<void> {
    const root = this.root;
    if (!root) return;

    this.saveState = "saving";

    try {
      await invoke("write_markdown", {
        root,
        path: pending.path,
        contents: pending.contents,
      });

      // A newer edit arrived while this write was in flight; it owns the state.
      if (this.#pending) return;

      this.dirty = false;
      this.saveState = "saved";
    } catch (error) {
      this.error = messageFrom(error);
      this.saveState = "error";
    }
  }
}

export const workspace = new Workspace();
