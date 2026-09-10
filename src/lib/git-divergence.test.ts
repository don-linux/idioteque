import { describe, expect, it } from "vitest";
import {
  branchPickerLabel,
  classifyDivergence,
  coincidenceHint,
  commitLabel,
} from "./git-divergence";

const commits = [
  { hash: "c2", parents: ["c1"] },
  { hash: "c1", parents: ["base"] },
  { hash: "o1", parents: ["base"] },
  { hash: "base", parents: ["root"] },
  { hash: "root", parents: [] },
];

describe("classifyDivergence", () => {
  it("highlights the merge-base and shared ancestors when the other branch is behind", () => {
    const marks = classifyDivergence(commits, "c2", [{ name: "old", mergeBase: "base" }]);

    expect(marks.get("base")).toEqual({ kind: "base", bases: ["old"] });
    expect(marks.get("root")?.kind).toBe("shared");
    expect(marks.get("c2")?.kind).toBe("current");
    expect(marks.get("c1")?.kind).toBe("current");
    expect(marks.get("o1")?.kind).toBe("other");
  });

  it("keeps the same merge-base when the other branch is ahead", () => {
    const ahead = [
      { hash: "o2", parents: ["o1"] },
      ...commits,
    ];
    const marks = classifyDivergence(ahead, "c2", [{ name: "feat", mergeBase: "base" }]);

    expect(marks.get("base")?.kind).toBe("base");
    expect(marks.get("o2")?.kind).toBe("other");
    expect(marks.get("c2")?.kind).toBe("current");
  });

  it("treats history as current-only when nothing is compared", () => {
    const marks = classifyDivergence(commits, "c2", []);
    expect(marks.get("c2")?.kind).toBe("current");
    expect(marks.get("base")?.kind).toBe("current");
    expect(marks.get("o1")?.kind).toBe("other");
  });

  it("can mark one commit as the base of several selected branches", () => {
    const marks = classifyDivergence(commits, "c2", [
      { name: "a", mergeBase: "base" },
      { name: "b", mergeBase: "base" },
    ]);
    expect(marks.get("base")).toEqual({ kind: "base", bases: ["a", "b"] });
  });
});

describe("commitLabel", () => {
  it("uses the standard short hash in parentheses", () => {
    expect(commitLabel("First commit", "d454323")).toBe("First commit (d454323)");
    expect(commitLabel("  ", "abc1234")).toBe("commit (abc1234)");
  });
});

describe("branchPickerLabel", () => {
  it("shows the current branch and how many extras are compared", () => {
    expect(branchPickerLabel("main", [], false)).toBe("main");
    expect(branchPickerLabel("main", ["feat"], false)).toBe("main + 1");
    expect(branchPickerLabel(null, ["feat", "old"], true)).toBe("HEAD + 2");
    expect(branchPickerLabel(null, [], false)).toBe("Ramas");
  });
});

describe("coincidenceHint", () => {
  it("names the compared branches", () => {
    expect(coincidenceHint([])).toBe("Coinciden hasta aquí");
    expect(coincidenceHint(["feat"])).toBe("Coinciden con feat hasta aquí");
    expect(coincidenceHint(["a", "b"])).toBe("Coinciden con a, b hasta aquí");
  });
});
