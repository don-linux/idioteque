import { describe, expect, it } from "vitest";
import {
  DEFAULT_FOOTER_ACTION_ORDER,
  footerActionIntent,
  runFooterAction,
} from "./footer-actions";

describe("DEFAULT_FOOTER_ACTION_ORDER", () => {
  it("is the fixed code-defined order", () => {
    expect(DEFAULT_FOOTER_ACTION_ORDER).toEqual([
      "home",
      "folder",
      "settings",
      "terminal",
      "git",
    ]);
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
