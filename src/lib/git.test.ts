import { describe, expect, it } from "vitest";
import {
  hasStagedChange,
  hasUnstagedChange,
  isConflict,
  isUntracked,
  type GitFile,
} from "./git";

function file(partial: Partial<GitFile> & Pick<GitFile, "kind">): GitFile {
  return {
    path: "a.md",
    staged: ".",
    unstaged: ".",
    ...partial,
  };
}

describe("git file columns", () => {
  it("treats untracked as unstaged only", () => {
    const untracked = file({ kind: "untracked", unstaged: "?" });
    expect(isUntracked(untracked)).toBe(true);
    expect(hasStagedChange(untracked)).toBe(false);
    expect(hasUnstagedChange(untracked)).toBe(true);
  });

  it("reads XY: staged M and clean worktree", () => {
    const staged = file({ kind: "ordinary", staged: "M", unstaged: "." });
    expect(hasStagedChange(staged)).toBe(true);
    expect(hasUnstagedChange(staged)).toBe(false);
  });

  it("reads XY: dirty worktree", () => {
    const dirty = file({ kind: "ordinary", staged: ".", unstaged: "M" });
    expect(hasStagedChange(dirty)).toBe(false);
    expect(hasUnstagedChange(dirty)).toBe(true);
  });

  it("flags unmerged as a conflict", () => {
    const conflict = file({ kind: "unmerged", staged: "U", unstaged: "U" });
    expect(isConflict(conflict)).toBe(true);
  });

  it("does not treat a conflict as untracked", () => {
    const conflict = file({ kind: "unmerged", staged: "U", unstaged: "U" });
    expect(isUntracked(conflict)).toBe(false);
    expect(hasStagedChange(conflict)).toBe(true);
    expect(hasUnstagedChange(conflict)).toBe(true);
  });

  it("reads ignored as unstaged, never staged or untracked", () => {
    const ignored = file({ kind: "ignored", unstaged: "!" });
    expect(isUntracked(ignored)).toBe(false);
    expect(isConflict(ignored)).toBe(false);
    expect(hasStagedChange(ignored)).toBe(false);
    expect(hasUnstagedChange(ignored)).toBe(true);
  });

  it("reads both staged and unstaged columns on a rename", () => {
    const renamed = file({
      kind: "renamed",
      path: "new.md",
      originalPath: "old.md",
      staged: "R",
      unstaged: "M",
    });
    expect(isUntracked(renamed)).toBe(false);
    expect(hasStagedChange(renamed)).toBe(true);
    expect(hasUnstagedChange(renamed)).toBe(true);
  });

  it("ignores a lying staged column on untracked files", () => {
    const lying = file({ kind: "untracked", staged: "M", unstaged: "?" });
    expect(isUntracked(lying)).toBe(true);
    expect(hasStagedChange(lying)).toBe(false);
    expect(hasUnstagedChange(lying)).toBe(true);
  });
});
