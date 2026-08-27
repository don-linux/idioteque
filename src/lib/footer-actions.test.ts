import { describe, expect, it } from "vitest";
import {
  DEFAULT_FOOTER_ACTION_ORDER,
  footerActionIntent,
  moveFooterAction,
  normalizeFooterOrder,
  runFooterAction,
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

describe("footerActionIntent", () => {
  it("keeps home, folder, settings, and terminal as real actions", () => {
    expect(footerActionIntent("home")).toBe("home");
    expect(footerActionIntent("folder")).toBe("folder");
    expect(footerActionIntent("settings")).toBe("settings");
    expect(footerActionIntent("terminal")).toBe("terminal");
  });

  it("marks git as idle so a click cannot be wired like terminal", () => {
    expect(footerActionIntent("git")).toBe("idle");
    expect(footerActionIntent("git")).not.toBe("terminal");
    expect(footerActionIntent("git")).not.toBe("home");
    expect(footerActionIntent("git")).not.toBe("folder");
    expect(footerActionIntent("git")).not.toBe("settings");
  });

  it("does not invoke any action when the icon is git", () => {
    const home = (): void => {
      throw new Error("home");
    };
    const folder = (): void => {
      throw new Error("folder");
    };
    const terminal = (): void => {
      throw new Error("terminal");
    };

    expect(() => runFooterAction("git", { home, folder, terminal })).not.toThrow();
  });

  it("routes the live icons and ignores settings (it is a link)", () => {
    const calls: string[] = [];
    const actions = {
      home: () => calls.push("home"),
      folder: () => calls.push("folder"),
      terminal: () => calls.push("terminal"),
    };

    runFooterAction("home", actions);
    runFooterAction("folder", actions);
    runFooterAction("terminal", actions);
    runFooterAction("settings", actions);
    runFooterAction("git", actions);

    expect(calls).toEqual(["home", "folder", "terminal"]);
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
