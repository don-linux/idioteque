import { SvelteSet } from "svelte/reactivity";
import {
  ancestorsOf,
  type DraftKind,
  type TreeDraft,
} from "$lib/file-tree";

/**
 * What the tree shows: which folders are open and the row being typed into.
 * It lives outside the component so hiding the panel does not collapse everything.
 */
class FileTreeView {
  expanded = new SvelteSet<string>();
  draft = $state<TreeDraft | null>(null);
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
    this.revealFolder(parent);
    this.draftError = null;
    this.draft = { kind, parent };
  }

  cancelDraft(): void {
    this.draft = null;
    this.draftError = null;
  }

  failDraft(message: string): void {
    this.draftError = message;
  }

  reset(): void {
    this.expanded.clear();
    this.draft = null;
    this.draftError = null;
  }
}

export const fileTree = new FileTreeView();
