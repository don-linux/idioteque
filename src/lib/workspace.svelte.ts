import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { SvelteMap } from "svelte/reactivity";
import { appConfig } from "$lib/app-config.svelte";
import { addTab, nextActiveAfterClose, removeTab } from "$lib/editor-tabs";
import { editorSession } from "$lib/editor-session.svelte";
import {
  baseNameOf,
  folderNameOf,
  joinTreePath,
  normalizeNewName,
  parentDirOf,
  siblingExists,
  type DraftKind,
} from "$lib/file-tree";
import { fileTree } from "$lib/file-tree.svelte";
import { terminal } from "$lib/terminal.svelte";
import { unsavedExit } from "$lib/unsaved-exit.svelte";
import { collectDraftWrites } from "$lib/workspace-save";

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

function treeHasFile(nodes: TreeNode[], path: string): boolean {
  for (const node of nodes) {
    if (node.kind === "file" && node.path === path) return true;
    if (node.kind === "dir" && treeHasFile(node.children, path)) return true;
  }

  return false;
}

function treeHasDir(nodes: TreeNode[], path: string): boolean {
  for (const node of nodes) {
    if (node.kind !== "dir") continue;
    if (node.path === path) return true;
    if (treeHasDir(node.children, path)) return true;
  }

  return false;
}


class Workspace {
  root = $state<string | null>(null);
  tree = $state<TreeNode[]>([]);
  openTabs = $state<string[]>([]);
  currentPath = $state<string | null>(null);
  content = $state("");
  dirty = $state(false);
  saveState = $state<SaveState>("idle");
  error = $state<string | null>(null);

  #drafts = new SvelteMap<string, string>();
  #contentFor = $state<string | null>(null);
  #writing: Promise<boolean> = Promise.resolve(true);
  /// Guards against a slow read landing after the user picked another file.
  #loadToken = 0;
  #unlisten: UnlistenFn | null = null;

  get hasEntries(): boolean {
    return this.tree.length > 0;
  }

  get folderName(): string {
    return this.root === null ? "" : folderNameOf(this.root);
  }

  isDirectory(path: string): boolean {
    return treeHasDir(this.tree, path);
  }

  get hasUnsaved(): boolean {
    return this.#drafts.size > 0;
  }

  get contentReady(): boolean {
    return this.currentPath !== null && this.#contentFor === this.currentPath;
  }

  hasDraft(path: string): boolean {
    return this.#drafts.has(path);
  }

  async openFolder(): Promise<void> {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return;

    await this.openRoot(selected);
  }

  async openRoot(path: string): Promise<void> {
    if (!(await this.#confirmDiscardUnsaved())) return;

    await this.#leaveSession();

    try {
      this.tree = await invoke<TreeNode[]>("list_context_tree", { root: path });
      this.error = null;
    } catch (error) {
      this.error = messageFrom(error);
      return;
    }

    this.root = path;
    this.#resetOpenFiles();

    const recorded = await appConfig.record(path);
    if (recorded) {
      this.root = recorded;
    }

    await this.#startWatch(this.root);
  }

  async closeWorkspace(): Promise<boolean> {
    if (!(await this.#confirmDiscardUnsaved())) return false;

    await this.#leaveSession();

    this.root = null;
    this.tree = [];
    this.#resetOpenFiles();
    this.error = null;
    return true;
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
    if (!root) return;

    this.openTabs = addTab(this.openTabs, path);
    fileTree.reveal(path);
    if (path === this.currentPath) return;

    const token = ++this.#loadToken;
    this.currentPath = path;
    this.error = null;

    const draft = this.#drafts.get(path);
    if (draft !== undefined) {
      this.content = draft;
      this.#contentFor = path;
      this.dirty = true;
      this.saveState = "idle";
      return;
    }

    this.dirty = false;
    this.saveState = "idle";
    this.#contentFor = null;

    try {
      const contents = await invoke<string>("read_markdown", { root, path });
      if (token !== this.#loadToken) return;
      this.content = contents;
      this.#contentFor = path;
    } catch (error) {
      if (token !== this.#loadToken) return;
      this.content = "";
      this.#contentFor = path;
      this.error = messageFrom(error);
    }
  }

  async closeTab(path: string): Promise<void> {
    if (!this.openTabs.includes(path)) return;

    if (this.#drafts.has(path)) {
      const confirmed = await unsavedExit.request("tab");
      if (!confirmed) return;
      this.#dropDraft(path);
    }

    this.#forgetTab(path);
  }

  async deleteFile(path: string): Promise<void> {
    const root = this.root;
    if (!root) return;

    const confirmed = await ask(`¿Borrar ${path}?`, {
      title: "idioteque",
      kind: "warning",
    });
    if (!confirmed) return;

    this.#dropDraft(path);

    try {
      await invoke("delete_markdown", { root, path });
      this.error = null;
    } catch (error) {
      this.error = messageFrom(error);
      return;
    }

    this.#forgetTab(path);

    await this.refreshTree();
  }

  /**
   * Creates a file or folder from the inline row of the tree. Failures stay on the
   * draft row instead of the sidebar banner, so the user can fix the name in place.
   */
  async createEntry(kind: DraftKind, parent: string, rawName: string): Promise<boolean> {
    const root = this.root;
    if (!root) return false;

    const normalized = normalizeNewName(rawName, kind);
    if (!normalized.ok) {
      fileTree.failDraft(normalized.error);
      return false;
    }

    const path = joinTreePath(parent, normalized.name);
    const name = baseNameOf(path);

    if (siblingExists(this.tree, parentDirOf(path), name)) {
      fileTree.failDraft(`\`${name}\` ya existe`);
      return false;
    }

    try {
      await invoke(kind === "file" ? "create_markdown" : "create_directory", { root, path });
      this.error = null;
    } catch (error) {
      fileTree.failDraft(messageFrom(error));
      return false;
    }

    fileTree.cancelDraft();
    await this.refreshTree();

    if (kind === "dir") {
      fileTree.revealFolder(path);
      return true;
    }

    fileTree.reveal(path);
    await this.openFile(path);
    return true;
  }

  async reloadFromDisk(): Promise<void> {
    const root = this.root;
    if (!root) return;

    const path = this.currentPath;
    await this.refreshTree();

    const missing = this.openTabs.filter((tab) => !treeHasFile(this.tree, tab));
    if (missing.length > 0) {
      const remaining = this.openTabs.filter((tab) => treeHasFile(this.tree, tab));
      for (const tab of missing) {
        this.#dropDraft(tab);
        editorSession.dropState(tab);
      }
      this.openTabs = remaining;

      if (path !== null && missing.includes(path) && this.currentPath === path) {
        if (remaining.length > 0) {
          await this.openFile(remaining[0]);
        } else {
          this.#dropOpenFile();
        }
        return;
      }
    }

    if (path === null || this.currentPath !== path) return;

    if (!treeHasFile(this.tree, path)) {
      this.#dropDraft(path);
      this.#dropOpenFile();
      return;
    }

    if (this.dirty) return;

    try {
      const contents = await invoke<string>("read_markdown", { root, path });
      if (this.currentPath !== path || this.dirty) return;
      if (contents === this.content) return;
      this.content = contents;
      this.#contentFor = path;
    } catch (error) {
      if (this.currentPath !== path) return;
      this.#dropDraft(path);
      this.#forgetTab(path);
      this.error = messageFrom(error);
    }
  }

  edit(contents: string): void {
    const path = this.currentPath;
    if (!path) return;

    this.content = contents;
    this.dirty = true;
    this.#drafts.set(path, contents);
  }

  async save(): Promise<void> {
    const path = this.currentPath;
    if (!path || !this.dirty) return;

    this.#writing = this.#writing.then(() =>
      this.#write({ path, contents: this.content }),
    );
    await this.#writing;
  }

  async saveAll(): Promise<boolean> {
    const writes = collectDraftWrites(this.#drafts);
    if (writes.length === 0) return true;

    for (const pending of writes) {
      this.#writing = this.#writing.then(() => this.#write(pending));
      if (!(await this.#writing)) return false;
    }

    return true;
  }

  discardUnsaved(): void {
    this.#clearDrafts();
    this.dirty = false;
    this.saveState = "idle";
  }

  #forgetTab(path: string): void {
    const next = nextActiveAfterClose(this.openTabs, path, this.currentPath);
    this.openTabs = removeTab(this.openTabs, path);
    editorSession.dropState(path);

    if (this.currentPath !== path) return;

    if (next) {
      void this.openFile(next);
      return;
    }

    this.#dropOpenFile();
  }

  #resetOpenFiles(): void {
    this.openTabs = [];
    editorSession.clearStates();
    fileTree.reset();
    this.#dropOpenFile();
  }

  #dropOpenFile(): void {
    this.currentPath = null;
    this.content = "";
    this.#contentFor = null;
    this.dirty = false;
    this.saveState = "idle";
    this.#loadToken += 1;
  }

  #dropDraft(path: string): void {
    this.#drafts.delete(path);
    if (this.currentPath === path) {
      this.dirty = false;
    }
  }

  #clearDrafts(): void {
    this.#drafts.clear();
    this.dirty = false;
  }

  async #confirmDiscardUnsaved(): Promise<boolean> {
    if (!this.hasUnsaved) return true;

    const confirmed = await unsavedExit.request();
    if (!confirmed) return false;

    this.#clearDrafts();
    return true;
  }

  async #leaveSession(): Promise<void> {
    await this.#stopWatch();
    await terminal.teardown();
  }

  async #startWatch(root: string): Promise<void> {
    await this.#stopWatch();

    try {
      await invoke("watch_workspace", { root });
      this.#unlisten = await listen("workspace-fs", () => {
        void this.reloadFromDisk();
      });
    } catch (error) {
      this.error = messageFrom(error);
    }
  }

  async #stopWatch(): Promise<void> {
    if (this.#unlisten) {
      this.#unlisten();
      this.#unlisten = null;
    }

    try {
      await invoke("unwatch_workspace");
    } catch {
      // Watcher may already be gone.
    }
  }

  async #write(pending: PendingWrite): Promise<boolean> {
    const root = this.root;
    if (!root) return false;

    this.saveState = "saving";

    try {
      await invoke("write_markdown", {
        root,
        path: pending.path,
        contents: pending.contents,
      });

      const draft = this.#drafts.get(pending.path);
      if (draft !== undefined && draft !== pending.contents) {
        return true;
      }

      this.#drafts.delete(pending.path);

      if (this.currentPath === pending.path) {
        this.dirty = false;
        this.saveState = "saved";
      } else if (this.#drafts.size === 0) {
        this.saveState = "saved";
      }

      return true;
    } catch (error) {
      this.error = messageFrom(error);
      this.saveState = "error";
      return false;
    }
  }
}

export const workspace = new Workspace();
