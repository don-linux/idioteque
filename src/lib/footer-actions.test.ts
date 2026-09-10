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
  it("keeps home, folder, settings, terminal, and git as real actions", () => {
    expect(footerActionIntent("home")).toBe("home");
    expect(footerActionIntent("folder")).toBe("folder");
    expect(footerActionIntent("settings")).toBe("settings");
    expect(footerActionIntent("terminal")).toBe("terminal");
    expect(footerActionIntent("git")).toBe("git");
    expect(footerActionIntent("git")).not.toBe("idle");
    expect(footerActionIntent("git")).not.toBe("terminal");
  });

  it("routes git like the other live icons", () => {
    const calls: string[] = [];
    const actions = {
      home: () => calls.push("home"),
      folder: () => calls.push("folder"),
      terminal: () => calls.push("terminal"),
      git: () => calls.push("git"),
    };

    runFooterAction("home", actions);
    runFooterAction("folder", actions);
    runFooterAction("terminal", actions);
    runFooterAction("settings", actions);
    runFooterAction("git", actions);

    expect(calls).toEqual(["home", "folder", "terminal", "git"]);
  });

  it("invokes only git when the icon is git", () => {
    const actions = {
      home: (): void => {
        throw new Error("home");
      },
      folder: (): void => {
        throw new Error("folder");
      },
      terminal: (): void => {
        throw new Error("terminal");
      },
      git: (): void => undefined,
    };

    expect(() => runFooterAction("git", actions)).not.toThrow();
  });
});
