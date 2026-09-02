import { describe, expect, it } from "vitest";
import {
  ancestorsOf,
  baseNameOf,
  draftParentFor,
  flattenTree,
  hasMarkdownExtension,
  joinTreePath,
  normalizeNewName,
  parentDirOf,
  revealPath,
  siblingExists,
  toggleExpanded,
  type TreeRow,
} from "./file-tree";
import type { TreeNode } from "./workspace.svelte";

function dir(name: string, path: string, children: TreeNode[] = []): TreeNode {
  return { name, path, kind: "dir", children };
}

function file(name: string, path: string): TreeNode {
  return { name, path, kind: "file", children: [] };
}

/** docs/ (guia.md, sub/ (nota.md)), src/, README.md */
function tree(): TreeNode[] {
  return [
    dir("docs", "docs", [
      dir("sub", "docs/sub", [file("nota.md", "docs/sub/nota.md")]),
      file("guia.md", "docs/guia.md"),
    ]),
    dir("src", "src"),
    file("README.md", "README.md"),
  ];
}

function shape(rows: TreeRow[]): string[] {
  return rows.map((row) => `${row.depth}:${row.kind}:${row.name || row.draftKind}`);
}

describe("flattenTree", () => {
  it("shows root entries and keeps collapsed folders closed", () => {
    const rows = flattenTree(tree(), { expanded: new Set() });

    expect(shape(rows)).toEqual(["0:dir:docs", "0:dir:src", "0:file:README.md"]);
    expect(rows.every((row) => !row.expanded)).toBe(true);
  });

  it("expands only the folders in the set", () => {
    const rows = flattenTree(tree(), { expanded: new Set(["docs"]) });

    expect(shape(rows)).toEqual([
      "0:dir:docs",
      "1:dir:sub",
      "1:file:guia.md",
      "0:dir:src",
      "0:file:README.md",
    ]);
    expect(rows[0].expanded).toBe(true);
    expect(rows[1].expanded).toBe(false);
  });

  it("nests deeper folders one level at a time", () => {
    const rows = flattenTree(tree(), { expanded: new Set(["docs", "docs/sub"]) });

    expect(shape(rows)).toEqual([
      "0:dir:docs",
      "1:dir:sub",
      "2:file:nota.md",
      "1:file:guia.md",
      "0:dir:src",
      "0:file:README.md",
    ]);
  });

  it("does not expand a child whose parent is collapsed", () => {
    const rows = flattenTree(tree(), { expanded: new Set(["docs/sub"]) });

    expect(shape(rows)).toEqual(["0:dir:docs", "0:dir:src", "0:file:README.md"]);
  });

  it("puts a root draft first", () => {
    const rows = flattenTree(tree(), {
      expanded: new Set(),
      draft: { kind: "file", parent: "" },
    });

    expect(shape(rows)[0]).toBe("0:draft:file");
    expect(rows[0].path).toBe("");
    expect(rows).toHaveLength(4);
  });

  it("puts a draft inside its expanded parent", () => {
    const rows = flattenTree(tree(), {
      expanded: new Set(["docs"]),
      draft: { kind: "dir", parent: "docs" },
    });

    expect(shape(rows)).toEqual([
      "0:dir:docs",
      "1:draft:dir",
      "1:dir:sub",
      "1:file:guia.md",
      "0:dir:src",
      "0:file:README.md",
    ]);
  });

  it("still shows a draft whose parent is collapsed", () => {
    const rows = flattenTree(tree(), {
      expanded: new Set(),
      draft: { kind: "file", parent: "docs" },
    });

    expect(shape(rows)).toEqual([
      "0:dir:docs",
      "1:draft:file",
      "0:dir:src",
      "0:file:README.md",
    ]);
  });

  it("falls back to the root when the draft parent is gone", () => {
    const rows = flattenTree(tree(), {
      expanded: new Set(),
      draft: { kind: "file", parent: "borrada" },
    });

    const drafts = rows.filter((row) => row.kind === "draft");
    expect(drafts).toHaveLength(1);
    expect(drafts[0].depth).toBe(0);
  });

  it("emits a single draft row even for a deeply nested parent", () => {
    const rows = flattenTree(tree(), {
      expanded: new Set(["docs", "docs/sub"]),
      draft: { kind: "file", parent: "docs/sub" },
    });

    expect(rows.filter((row) => row.kind === "draft")).toHaveLength(1);
    expect(shape(rows)[2]).toBe("2:draft:file");
  });

  it("handles an empty tree with and without a draft", () => {
    expect(flattenTree([], { expanded: new Set() })).toEqual([]);
    expect(shape(flattenTree([], { expanded: new Set(), draft: { kind: "dir", parent: "" } }))).toEqual(
      ["0:draft:dir"],
    );
  });

  it("shows folders that hold no markdown at all", () => {
    const rows = flattenTree([dir("assets", "assets")], { expanded: new Set(["assets"]) });

    expect(shape(rows)).toEqual(["0:dir:assets"]);
    expect(rows[0].expanded).toBe(true);
  });
});

describe("toggleExpanded", () => {
  it("adds a collapsed path and removes an expanded one", () => {
    const first = toggleExpanded(new Set(), "docs");
    expect([...first]).toEqual(["docs"]);

    const second = toggleExpanded(first, "docs");
    expect([...second]).toEqual([]);
  });

  it("does not mutate the input", () => {
    const original = new Set(["docs"]);
    toggleExpanded(original, "src");
    expect([...original]).toEqual(["docs"]);
  });
});

describe("ancestorsOf / revealPath", () => {
  it("lists ancestors outermost first, excluding the entry itself", () => {
    expect(ancestorsOf("a/b/c.md")).toEqual(["a", "a/b"]);
    expect(ancestorsOf("README.md")).toEqual([]);
    expect(ancestorsOf("")).toEqual([]);
  });

  it("expands every ancestor and keeps what was open", () => {
    const next = revealPath(new Set(["otra"]), "a/b/c.md");
    expect([...next].sort()).toEqual(["a", "a/b", "otra"]);
  });

  it("is a no-op for a root entry", () => {
    const next = revealPath(new Set(), "README.md");
    expect([...next]).toEqual([]);
  });
});

describe("parentDirOf / baseNameOf / joinTreePath", () => {
  it("splits a relative tree path", () => {
    expect(parentDirOf("a/b/c.md")).toBe("a/b");
    expect(parentDirOf("README.md")).toBe("");
    expect(baseNameOf("a/b/c.md")).toBe("c.md");
    expect(baseNameOf("README.md")).toBe("README.md");
  });

  it("joins without a leading slash at the root", () => {
    expect(joinTreePath("", "nota.md")).toBe("nota.md");
    expect(joinTreePath("docs", "nota.md")).toBe("docs/nota.md");
  });

  it("round trips", () => {
    const path = joinTreePath(parentDirOf("a/b/c.md"), baseNameOf("a/b/c.md"));
    expect(path).toBe("a/b/c.md");
  });
});

describe("draftParentFor", () => {
  const isDirectory = (path: string) => path === "docs" || path === "docs/sub";

  it("uses the root when nothing is selected", () => {
    expect(draftParentFor(null, isDirectory)).toBe("");
  });

  it("creates inside the selected folder", () => {
    expect(draftParentFor("docs", isDirectory)).toBe("docs");
    expect(draftParentFor("docs/sub", isDirectory)).toBe("docs/sub");
  });

  it("creates next to the selected file", () => {
    expect(draftParentFor("docs/guia.md", isDirectory)).toBe("docs");
    expect(draftParentFor("README.md", isDirectory)).toBe("");
  });
});

describe("normalizeNewName", () => {
  it("adds .md to files only", () => {
    expect(normalizeNewName("nota", "file")).toEqual({ ok: true, name: "nota.md" });
    expect(normalizeNewName("nota.md", "file")).toEqual({ ok: true, name: "nota.md" });
    expect(normalizeNewName("nota.MD", "file")).toEqual({ ok: true, name: "nota.MD" });
    expect(normalizeNewName("notas", "dir")).toEqual({ ok: true, name: "notas" });
  });

  it("keeps other extensions and appends .md", () => {
    expect(normalizeNewName("script.ts", "file")).toEqual({ ok: true, name: "script.ts.md" });
  });

  it("trims spaces and surrounding slashes", () => {
    expect(normalizeNewName("  nota  ", "file")).toEqual({ ok: true, name: "nota.md" });
    expect(normalizeNewName("/notas/", "dir")).toEqual({ ok: true, name: "notas" });
  });

  it("allows a nested name", () => {
    expect(normalizeNewName("2026/enero", "dir")).toEqual({ ok: true, name: "2026/enero" });
    expect(normalizeNewName("sub/nota", "file")).toEqual({ ok: true, name: "sub/nota.md" });
  });

  it("rejects empty names", () => {
    expect(normalizeNewName("", "file").ok).toBe(false);
    expect(normalizeNewName("   ", "dir").ok).toBe(false);
    expect(normalizeNewName("///", "dir").ok).toBe(false);
    expect(normalizeNewName(".md", "file").ok).toBe(false);
  });

  it("rejects traversal, backslashes and empty segments", () => {
    expect(normalizeNewName("..", "dir").ok).toBe(false);
    expect(normalizeNewName("../fuera", "file").ok).toBe(false);
    expect(normalizeNewName("docs/../fuera", "file").ok).toBe(false);
    expect(normalizeNewName(".", "dir").ok).toBe(false);
    expect(normalizeNewName("a/./b", "dir").ok).toBe(false);
    expect(normalizeNewName("a//b", "dir").ok).toBe(false);
    expect(normalizeNewName("a/ /b", "dir").ok).toBe(false);
    expect(normalizeNewName("C:\\temp", "file").ok).toBe(false);
    expect(normalizeNewName("a\\b", "dir").ok).toBe(false);
  });

  it("keeps dotfiles, which the tree does show", () => {
    expect(normalizeNewName(".cursor", "dir")).toEqual({ ok: true, name: ".cursor" });
    expect(normalizeNewName(".agents/persona", "file")).toEqual({
      ok: true,
      name: ".agents/persona.md",
    });
  });
});

describe("hasMarkdownExtension", () => {
  it("ignores case", () => {
    expect(hasMarkdownExtension("a.md")).toBe(true);
    expect(hasMarkdownExtension("a.MD")).toBe(true);
    expect(hasMarkdownExtension("a.markdown")).toBe(false);
    expect(hasMarkdownExtension("md")).toBe(false);
  });
});

describe("siblingExists", () => {
  it("finds collisions at the root and inside folders", () => {
    expect(siblingExists(tree(), "", "README.md")).toBe(true);
    expect(siblingExists(tree(), "", "docs")).toBe(true);
    expect(siblingExists(tree(), "", "otro.md")).toBe(false);
    expect(siblingExists(tree(), "docs", "guia.md")).toBe(true);
    expect(siblingExists(tree(), "docs/sub", "nota.md")).toBe(true);
    expect(siblingExists(tree(), "docs/sub", "otra.md")).toBe(false);
  });

  it("ignores case, since two cased names would collide on macOS", () => {
    expect(siblingExists(tree(), "", "readme.md")).toBe(true);
    expect(siblingExists(tree(), "docs", "GUIA.MD")).toBe(true);
  });

  it("reports no collision for an unknown parent", () => {
    expect(siblingExists(tree(), "nueva", "nota.md")).toBe(false);
    expect(siblingExists(tree(), "src", "nota.md")).toBe(false);
  });
});
