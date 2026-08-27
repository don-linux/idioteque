import { describe, expect, it } from "vitest";
import {
  DEFAULT_FOOTER_ACTION_ORDER,
  moveFooterAction,
  normalizeFooterOrder,
} from "./footer-actions";

describe("normalizeFooterOrder", () => {
  it("uses the default when the saved list is missing or empty", () => {
    expect(normalizeFooterOrder(undefined)).toEqual(DEFAULT_FOOTER_ACTION_ORDER);
    expect(normalizeFooterOrder(null)).toEqual(DEFAULT_FOOTER_ACTION_ORDER);
    expect(normalizeFooterOrder([])).toEqual(DEFAULT_FOOTER_ACTION_ORDER);
    expect(DEFAULT_FOOTER_ACTION_ORDER).toEqual(["home", "folder", "settings", "terminal", "git"]);
  });

  it("keeps a valid custom order that already includes git", () => {
    expect(normalizeFooterOrder(["git", "terminal", "home", "settings", "folder"])).toEqual([
      "git",
      "terminal",
      "home",
      "settings",
      "folder",
    ]);
  });

  it("drops unknown ids and duplicates", () => {
    expect(
      normalizeFooterOrder(["home", "ghost", "home", "terminal", "folder", "settings", "git", "git"]),
    ).toEqual(["home", "terminal", "folder", "settings", "git"]);
  });

  it("appends git to a pre-git saved order without reshuffling it", () => {
    expect(normalizeFooterOrder(["terminal", "home", "settings", "folder"])).toEqual([
      "terminal",
      "home",
      "settings",
      "folder",
      "git",
    ]);
  });

  it("appends new default ids that are missing from the saved order", () => {
    expect(normalizeFooterOrder(["terminal", "home"])).toEqual([
      "terminal",
      "home",
      "folder",
      "settings",
      "git",
    ]);
  });

  it("ignores future ids that the app does not know yet", () => {
    expect(normalizeFooterOrder(["search", "home", "folder", "settings", "terminal", "git"])).toEqual(
      DEFAULT_FOOTER_ACTION_ORDER,
    );
  });

  it("does not drop terminal when inserting git", () => {
    const next = normalizeFooterOrder(["home", "folder", "settings", "terminal"]);
    expect(next).toContain("terminal");
    expect(next).toContain("git");
    expect(next.filter((id) => id === "terminal")).toHaveLength(1);
  });
});

describe("moveFooterAction", () => {
  it("moves an item to another index", () => {
    expect(moveFooterAction(["home", "folder", "settings", "terminal", "git"], 0, 2)).toEqual([
      "folder",
      "settings",
      "home",
      "terminal",
      "git",
    ]);
  });

  it("leaves the order alone when the index is out of range", () => {
    const order = ["home", "folder", "settings", "terminal", "git"] as const;
    expect(moveFooterAction(order, -1, 1)).toEqual([...order]);
    expect(moveFooterAction(order, 0, 9)).toEqual([...order]);
  });
});
