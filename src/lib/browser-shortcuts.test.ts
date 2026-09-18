import { describe, expect, it, vi } from "vitest";
import { isTerminalTarget } from "./panel-shortcuts";
import {
  handleBrowserShortcut,
  isBrowserToggleAnywhereShortcut,
  isBrowserToggleShortcut,
} from "./browser-shortcuts";

function key(
  partial: Partial<{
    code: string;
    ctrlKey: boolean;
    metaKey: boolean;
    shiftKey: boolean;
    altKey: boolean;
  }> = {},
) {
  return {
    code: "KeyB",
    ctrlKey: true,
    metaKey: false,
    shiftKey: false,
    altKey: false,
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
    stopImmediatePropagation: vi.fn(),
    ...partial,
  };
}

describe("isBrowserToggleShortcut", () => {
  it("matches only plain Ctrl+B", () => {
    expect(isBrowserToggleShortcut(key())).toBe(true);
    expect(isBrowserToggleShortcut(key({ code: "KeyJ" }))).toBe(false);
    expect(isBrowserToggleShortcut(key({ code: "KeyT" }))).toBe(false);
    expect(isBrowserToggleShortcut(key({ ctrlKey: false }))).toBe(false);
    expect(isBrowserToggleShortcut(key({ metaKey: true }))).toBe(false);
    expect(isBrowserToggleShortcut(key({ shiftKey: true }))).toBe(false);
    expect(isBrowserToggleShortcut(key({ altKey: true }))).toBe(false);
  });

  // `code` has to match whole, not by suffix or substring: a looser comparison
  // would hand unrelated physical keys the browser toggle.
  it("compares the whole code, not part of it", () => {
    for (const code of ["IntlB", "KeyBB", "BracketLeftB", "keyb", "Key", "B", ""]) {
      expect(isBrowserToggleShortcut(key({ code }))).toBe(false);
      expect(isBrowserToggleAnywhereShortcut(key({ code, shiftKey: true }))).toBe(false);
    }
  });
});

describe("isBrowserToggleAnywhereShortcut", () => {
  it("matches only Ctrl+Shift+B", () => {
    expect(isBrowserToggleAnywhereShortcut(key({ shiftKey: true }))).toBe(true);
    expect(isBrowserToggleAnywhereShortcut(key())).toBe(false);
    expect(isBrowserToggleAnywhereShortcut(key({ shiftKey: true, altKey: true }))).toBe(
      false,
    );
    expect(isBrowserToggleAnywhereShortcut(key({ shiftKey: true, metaKey: true }))).toBe(
      false,
    );
    expect(isBrowserToggleAnywhereShortcut(key({ code: "KeyT", shiftKey: true }))).toBe(
      false,
    );
  });

  it("does not overlap with the plain chord", () => {
    expect(isBrowserToggleShortcut(key({ shiftKey: true }))).toBe(false);
    expect(isBrowserToggleAnywhereShortcut(key())).toBe(false);
  });
});

describe("handleBrowserShortcut", () => {
  it("toggles the browser and swallows the chord", () => {
    const event = key();
    const toggleBrowser = vi.fn();

    handleBrowserShortcut(event, {
      hasWorkspace: true,
      insideTerminal: false,
      toggleBrowser,
    });

    expect(toggleBrowser).toHaveBeenCalledTimes(1);
    expect(event.preventDefault).toHaveBeenCalledTimes(1);
    expect(event.stopPropagation).toHaveBeenCalledTimes(1);
    expect(event.stopImmediatePropagation).toHaveBeenCalledTimes(1);
  });

  it("is a no-op without a workspace", () => {
    const event = key();
    const toggleBrowser = vi.fn();

    handleBrowserShortcut(event, {
      hasWorkspace: false,
      insideTerminal: false,
      toggleBrowser,
    });

    expect(toggleBrowser).not.toHaveBeenCalled();
    expect(event.preventDefault).not.toHaveBeenCalled();
  });

  it("leaves Ctrl+B alone inside the terminal, so tmux keeps its prefix", () => {
    const event = key();
    const toggleBrowser = vi.fn();

    handleBrowserShortcut(event, {
      hasWorkspace: true,
      insideTerminal: true,
      toggleBrowser,
    });

    expect(toggleBrowser).not.toHaveBeenCalled();
    expect(event.preventDefault).not.toHaveBeenCalled();
    expect(event.stopPropagation).not.toHaveBeenCalled();
  });

  it("still toggles from inside the terminal with Ctrl+Shift+B", () => {
    const event = key({ shiftKey: true });
    const toggleBrowser = vi.fn();

    handleBrowserShortcut(event, {
      hasWorkspace: true,
      insideTerminal: true,
      toggleBrowser,
    });

    expect(toggleBrowser).toHaveBeenCalledTimes(1);
    expect(event.preventDefault).toHaveBeenCalledTimes(1);
  });

  it("accepts Ctrl+Shift+B outside the terminal too", () => {
    const event = key({ shiftKey: true });
    const toggleBrowser = vi.fn();

    handleBrowserShortcut(event, {
      hasWorkspace: true,
      insideTerminal: false,
      toggleBrowser,
    });

    expect(toggleBrowser).toHaveBeenCalledTimes(1);
  });

  it("ignores other chords without touching the event", () => {
    const event = key({ altKey: true });
    const toggleBrowser = vi.fn();

    handleBrowserShortcut(event, {
      hasWorkspace: true,
      insideTerminal: false,
      toggleBrowser,
    });

    expect(toggleBrowser).not.toHaveBeenCalled();
    expect(event.preventDefault).not.toHaveBeenCalled();
  });

  it("does not fire without a workspace, even with the anywhere chord", () => {
    const event = key({ shiftKey: true });
    const toggleBrowser = vi.fn();

    handleBrowserShortcut(event, {
      hasWorkspace: false,
      insideTerminal: true,
      toggleBrowser,
    });

    expect(toggleBrowser).not.toHaveBeenCalled();
    expect(event.preventDefault).not.toHaveBeenCalled();
  });

  it("lets tmux keep Ctrl+B when the event came from an xterm surface", () => {
    const target = { closest: (selector: string) => (selector === ".xterm" ? {} : null) };
    const event = key();
    const toggleBrowser = vi.fn();

    handleBrowserShortcut(event, {
      hasWorkspace: true,
      insideTerminal: isTerminalTarget(target as unknown as EventTarget),
      toggleBrowser,
    });

    expect(toggleBrowser).not.toHaveBeenCalled();
    expect(event.preventDefault).not.toHaveBeenCalled();

    const anywhere = key({ shiftKey: true });
    handleBrowserShortcut(anywhere, {
      hasWorkspace: true,
      insideTerminal: isTerminalTarget(target as unknown as EventTarget),
      toggleBrowser,
    });

    expect(toggleBrowser).toHaveBeenCalledTimes(1);
  });

  it("toggles on Ctrl+B when the event came from the editor", () => {
    const target = { closest: () => null };
    const event = key();
    const toggleBrowser = vi.fn();

    handleBrowserShortcut(event, {
      hasWorkspace: true,
      insideTerminal: isTerminalTarget(target as unknown as EventTarget),
      toggleBrowser,
    });

    expect(toggleBrowser).toHaveBeenCalledTimes(1);
    expect(event.preventDefault).toHaveBeenCalledTimes(1);
  });

  it("still swallows the chord when stopImmediatePropagation is missing", () => {
    const { stopImmediatePropagation: _ignored, ...event } = key();
    const toggleBrowser = vi.fn();

    handleBrowserShortcut(event, {
      hasWorkspace: true,
      insideTerminal: false,
      toggleBrowser,
    });

    expect(toggleBrowser).toHaveBeenCalledTimes(1);
    expect(event.preventDefault).toHaveBeenCalledTimes(1);
    expect(event.stopPropagation).toHaveBeenCalledTimes(1);
  });

  it("does not treat a lone KeyB or the letter b as a chord", () => {
    const letter = key({ ctrlKey: false, code: "KeyB" });
    const short = key({ code: "b" });
    const toggleBrowser = vi.fn();

    handleBrowserShortcut(letter, {
      hasWorkspace: true,
      insideTerminal: false,
      toggleBrowser,
    });
    handleBrowserShortcut(short, {
      hasWorkspace: true,
      insideTerminal: false,
      toggleBrowser,
    });

    expect(toggleBrowser).not.toHaveBeenCalled();
    expect(letter.preventDefault).not.toHaveBeenCalled();
    expect(short.preventDefault).not.toHaveBeenCalled();
  });

  it("does not steal Ctrl+Alt+Shift+B or Ctrl+Shift+Meta+B", () => {
    const alt = key({ shiftKey: true, altKey: true });
    const meta = key({ shiftKey: true, metaKey: true });
    const toggleBrowser = vi.fn();

    handleBrowserShortcut(alt, {
      hasWorkspace: true,
      insideTerminal: true,
      toggleBrowser,
    });
    handleBrowserShortcut(meta, {
      hasWorkspace: true,
      insideTerminal: false,
      toggleBrowser,
    });

    expect(toggleBrowser).not.toHaveBeenCalled();
    expect(isBrowserToggleAnywhereShortcut(alt)).toBe(false);
    expect(isBrowserToggleAnywhereShortcut(meta)).toBe(false);
  });
});

describe("window chords", () => {
  it("does not treat Ctrl+L as a window toggle", () => {
    const event = key({ code: "KeyL" });
    const toggleBrowser = vi.fn();

    handleBrowserShortcut(event, {
      hasWorkspace: true,
      insideTerminal: false,
      toggleBrowser,
    });

    expect(toggleBrowser).not.toHaveBeenCalled();
    expect(event.preventDefault).not.toHaveBeenCalled();
  });
});
