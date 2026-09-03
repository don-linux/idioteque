import { SvelteSet } from "svelte/reactivity";
import {
  ancestorsOf,
  pathIsUnder,
  remapPathPrefix,
  type DraftKind,
  type TreeDraft,
} from "$lib/file-tree";

export interface TreeRename {
  path: string;
  kind: DraftKind;
}

/**
 * What the tree shows: which folders are open and the row being typed into.
 * It lives outside the component so hiding the panel does not collapse everything.
 */
class FileTreeView {
  expanded = new SvelteSet<string>();
  draft = $state<TreeDraft | null>(null);
  rename = $state<TreeRename | null>(null);
  draftError = $state<string | null>(null);

  isExpanded(path: string): boolean {
    return this.expanded.has(path);
  }

  toggle(path: string): void {
    if (!this.expanded.delete(path)) this.expanded.add(path);
  }

  expand(path: string): void {
    this.expanded.add(path);
  }

  /** Opens every folder above `path` so the entry becomes visible. */
  reveal(path: string): void {
    for (const ancestor of ancestorsOf(path)) this.expanded.add(ancestor);
  }

  /** Same as `reveal`, plus the folder itself, to show what is inside it. */
  revealFolder(path: string): void {
    if (!path) return;
    this.reveal(path);
    this.expanded.add(path);
  }

  collapseAll(): void {
    this.expanded.clear();
  }

  startDraft(kind: DraftKind, parent: string): void {
    this.cancelRename();
    this.revealFolder(parent);
    this.draftError = null;
    this.draft = { kind, parent };
  }

  cancelDraft(): void {
    this.draft = null;
    this.draftError = null;
  }

  startRename(path: string, kind: DraftKind): void {
    this.cancelDraft();
    this.reveal(path);
    this.draftError = null;
    this.rename = { path, kind };
  }

  cancelRename(): void {
    this.rename = null;
    this.draftError = null;
  }

  remapExpanded(from: string, to: string): void {
    const next = [...this.expanded].map((path) => remapPathPrefix(path, from, to));
    this.expanded.clear();
    for (const path of next) this.expanded.add(path);
  }

  dropExpandedUnder(folder: string): void {
    for (const path of [...this.expanded]) {
      if (pathIsUnder(path, folder)) this.expanded.delete(path);
    }
  }

  failDraft(message: string): void {
    this.draftError = message;
  }

  reset(): void {
    this.expanded.clear();
    this.draft = null;
    this.rename = null;
    this.draftError = null;
  }
}

export const fileTree = new FileTreeView();
