import type { TreeNode } from "$lib/workspace.svelte";

export const MARKDOWN_EXTENSION = ".md";

export type DraftKind = "file" | "dir";

/** An in-progress name the user is typing in the tree. `parent` is "" at the root. */
export interface TreeDraft {
  kind: DraftKind;
  parent: string;
}

export type TreeRowKind = "dir" | "file" | "draft";

export interface TreeRow {
  kind: TreeRowKind;
  /** Empty for the draft row: it has no path until the name is committed. */
  path: string;
  name: string;
  /** 0 at the root, one step per nesting level. */
  depth: number;
  expanded: boolean;
  draftKind?: DraftKind;
}

export interface FlattenOptions {
  expanded: ReadonlySet<string>;
  draft?: TreeDraft | null;
}

function draftRow(draft: TreeDraft, depth: number): TreeRow {
  return {
    kind: "draft",
    path: "",
    name: "",
    depth,
    expanded: false,
    draftKind: draft.kind,
  };
}

/**
 * The visible tree as a flat list, so rows can be full width and indented by
 * padding instead of nested lists. Collapsed directories hide their subtree.
 */
export function flattenTree(nodes: TreeNode[], options: FlattenOptions): TreeRow[] {
  const { expanded, draft = null } = options;
  const rows: TreeRow[] = [];

  function walk(children: TreeNode[], depth: number, parent: string): void {
    if (draft && draft.parent === parent) {
      rows.push(draftRow(draft, depth));
    }

    for (const node of children) {
      if (node.kind === "file") {
        rows.push({ kind: "file", path: node.path, name: node.name, depth, expanded: false });
        continue;
      }

      const open = expanded.has(node.path);
      rows.push({ kind: "dir", path: node.path, name: node.name, depth, expanded: open });

      if (open) walk(node.children, depth + 1, node.path);
    }
  }

  walk(nodes, 0, "");

  // A draft inside a collapsed or unknown folder would otherwise vanish.
  if (draft && draft.parent !== "" && !rows.some((row) => row.kind === "draft")) {
    const index = rows.findIndex((row) => row.kind === "dir" && row.path === draft.parent);
    if (index >= 0) {
      rows.splice(index + 1, 0, draftRow(draft, rows[index].depth + 1));
    } else {
      rows.push(draftRow(draft, 0));
    }
  }

  return rows;
}

export function toggleExpanded(expanded: ReadonlySet<string>, path: string): Set<string> {
  const next = new Set(expanded);
  if (!next.delete(path)) next.add(path);
  return next;
}

/** Every ancestor directory of a path, outermost first. `a/b/c.md` → `a`, `a/b`. */
export function ancestorsOf(path: string): string[] {
  const segments = path.split("/").filter((segment) => segment.length > 0);
  segments.pop();

  const ancestors: string[] = [];
  let current = "";

  for (const segment of segments) {
    current = current ? `${current}/${segment}` : segment;
    ancestors.push(current);
  }

  return ancestors;
}

/** Expands whatever it takes to make `path` visible. */
export function revealPath(expanded: ReadonlySet<string>, path: string): Set<string> {
  const next = new Set(expanded);
  for (const ancestor of ancestorsOf(path)) next.add(ancestor);
  return next;
}

export function parentDirOf(path: string): string {
  const index = path.lastIndexOf("/");
  return index <= 0 ? "" : path.slice(0, index);
}

export function baseNameOf(path: string): string {
  const index = path.lastIndexOf("/");
  return index < 0 ? path : path.slice(index + 1);
}

export function joinTreePath(parent: string, name: string): string {
  return parent ? `${parent}/${name}` : name;
}

/** Where a new entry lands when the user has a file or folder selected. */
export function draftParentFor(
  selected: string | null,
  isDirectory: (path: string) => boolean,
): string {
  if (!selected) return "";
  if (isDirectory(selected)) return selected;
  return parentDirOf(selected);
}

export function hasMarkdownExtension(name: string): boolean {
  return name.toLowerCase().endsWith(MARKDOWN_EXTENSION);
}

export type NewNameResult = { ok: true; name: string } | { ok: false; error: string };

/**
 * Cleans up what the user typed in the tree. Nested names like `a/b.md` are
 * allowed; traversal, separators that would escape, and dot names are not.
 */
export function normalizeNewName(raw: string, kind: DraftKind): NewNameResult {
  const trimmed = raw.trim().replace(/^\/+|\/+$/g, "");

  if (trimmed.length === 0) return { ok: false, error: "Escribe un nombre" };
  if (trimmed.includes("\\")) return { ok: false, error: "El nombre no puede llevar `\\`" };
  if (trimmed.includes("//")) return { ok: false, error: "Ruta inválida" };

  const segments = trimmed.split("/");
  if (segments.some((segment) => segment === "." || segment === "..")) {
    return { ok: false, error: "Ruta inválida" };
  }
  if (segments.some((segment) => segment.trim().length === 0)) {
    return { ok: false, error: "Ruta inválida" };
  }

  if (kind === "dir") return { ok: true, name: trimmed };

  const name = hasMarkdownExtension(trimmed) ? trimmed : `${trimmed}${MARKDOWN_EXTENSION}`;
  if (name === MARKDOWN_EXTENSION) return { ok: false, error: "Escribe un nombre" };

  return { ok: true, name };
}

function childrenAt(nodes: TreeNode[], parent: string): TreeNode[] {
  if (parent === "") return nodes;

  for (const node of nodes) {
    if (node.kind !== "dir") continue;
    if (node.path === parent) return node.children;
    if (parent.startsWith(`${node.path}/`)) return childrenAt(node.children, parent);
  }

  return [];
}

/** Case insensitive on purpose: macOS and Windows would collide anyway. */
export function siblingExists(nodes: TreeNode[], parent: string, name: string): boolean {
  const target = name.toLowerCase();
  return childrenAt(nodes, parent).some((node) => node.name.toLowerCase() === target);
}
