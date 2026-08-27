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
});
