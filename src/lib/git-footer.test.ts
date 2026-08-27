import { describe, expect, it } from "vitest";
import type { GitSnapshot } from "./git";
import {
  gitFooterStateFromError,
  gitFooterStateFromSnapshot,
  gitFooterTitle,
  repoDisplayName,
  type GitFooterState,
} from "./git-footer";

function snapshot(partial: {
  available?: boolean;
  repository?: GitSnapshot["repository"];
}): GitSnapshot {
  return {
    probe: {
      available: partial.available ?? true,
      path: partial.available === false ? undefined : "/usr/bin/git",
      version: partial.available === false ? undefined : "2.43.0",
    },
    repository: partial.repository,
  };
}

function repo(partial: Partial<NonNullable<GitSnapshot["repository"]>> = {}) {
  return {
    toplevel: "/workspace/idioteque",
    gitDir: "/workspace/idioteque/.git",
    detached: false,
    initial: false,
    ahead: 0,
    behind: 0,
    dirty: false,
    files: [],
    ...partial,
  };
}

describe("repoDisplayName", () => {
  it("keeps only the last path segment", () => {
    expect(repoDisplayName("/workspace/idioteque")).toBe("idioteque");
    expect(repoDisplayName("C:\\\\Users\\\\me\\\\notes")).toBe("notes");
    expect(repoDisplayName("/tmp/notas/")).toBe("notas");
  });

  it("does not return a path that still contains separators", () => {
    const name = repoDisplayName("/home/ubuntu/projects/idioteque");
    expect(name).toBe("idioteque");
    expect(name.includes("/")).toBe(false);
    expect(name.includes("\\")).toBe(false);
    expect(name.includes("home")).toBe(false);
  });
});

describe("gitFooterStateFromSnapshot", () => {
  it("treats a missing binary as unavailable, not as an empty folder", () => {
    const state = gitFooterStateFromSnapshot(snapshot({ available: false }));
    expect(state).toEqual({ kind: "unavailable" });
    expect(gitFooterTitle(state)).toBe("Git no está disponible");
    expect(gitFooterTitle(state)).not.toBe("Sin repositorio Git");
  });

  it("treats an available probe without a repository as empty", () => {
    const state = gitFooterStateFromSnapshot(snapshot({ available: true }));
    expect(state).toEqual({ kind: "empty" });
    expect(gitFooterTitle(state)).toBe("Sin repositorio Git");
  });

  it("shows the repo name and branch", () => {
    const state = gitFooterStateFromSnapshot(
      snapshot({ repository: repo({ toplevel: "/workspace/idioteque", branch: "main" }) }),
    );
    expect(state).toEqual({
      kind: "repo",
      name: "idioteque",
      branch: "main",
      detached: false,
    });
    expect(gitFooterTitle(state)).toBe("idioteque · main");
    expect(gitFooterTitle(state)).not.toContain("/workspace");
  });

  it("shows detached HEAD without inventing a branch", () => {
    const state = gitFooterStateFromSnapshot(
      snapshot({
        repository: repo({
          toplevel: "/tmp/scratch",
          branch: undefined,
          detached: true,
          oid: "abc123",
        }),
      }),
    );
    expect(state).toEqual({ kind: "repo", name: "scratch", detached: true });
    expect(gitFooterTitle(state)).toBe("scratch · HEAD separado");
    expect(gitFooterTitle(state)).not.toContain("abc123");
  });

  it("treats a blank branch as detached, not as ' · '", () => {
    const state = gitFooterStateFromSnapshot(
      snapshot({ repository: repo({ branch: "   ", detached: false }) }),
    );
    expect(state.kind).toBe("repo");
    expect(gitFooterTitle(state)).toBe("idioteque · HEAD separado");
    expect(gitFooterTitle(state)).not.toMatch(/·\s*$/);
  });

  it("ignores dirty files in the live tooltip", () => {
    const clean = gitFooterTitle(
      gitFooterStateFromSnapshot(snapshot({ repository: repo({ branch: "main", dirty: false }) })),
    );
    const dirty = gitFooterTitle(
      gitFooterStateFromSnapshot(
        snapshot({
          repository: repo({
            branch: "main",
            dirty: true,
            files: [
              {
                path: "a.md",
                staged: ".",
                unstaged: "M",
                kind: "ordinary",
              },
            ],
          }),
        }),
      ),
    );
    expect(dirty).toBe(clean);
    expect(dirty).toBe("idioteque · main");
    expect(dirty).not.toMatch(/dirty|a\.md|\+/i);
  });
});

describe("gitFooterTitle", () => {
  it("uses a generic label while loading", () => {
    expect(gitFooterTitle({ kind: "loading" })).toBe("Git");
  });

  it("does not dress an invoke error as 'no repo'", () => {
    const state = gitFooterStateFromError();
    expect(state).toEqual({ kind: "error" });
    expect(gitFooterTitle(state)).toBe("Git no responde");
    expect(gitFooterTitle(state)).not.toBe("Sin repositorio Git");
    expect(gitFooterTitle(state)).not.toBe("Git no está disponible");
  });

  it("never puts a full toplevel path in the title", () => {
    const states: GitFooterState[] = [
      gitFooterStateFromSnapshot(
        snapshot({ repository: repo({ toplevel: "/very/long/path/to/notes", branch: "dev" }) }),
      ),
      gitFooterStateFromSnapshot(
        snapshot({
          repository: repo({ toplevel: "/very/long/path/to/notes", detached: true, branch: undefined }),
        }),
      ),
    ];

    for (const state of states) {
      const title = gitFooterTitle(state);
      expect(title).not.toContain("/very");
      expect(title).not.toContain("long/path");
      expect(title.startsWith("notes")).toBe(true);
    }
  });
});
