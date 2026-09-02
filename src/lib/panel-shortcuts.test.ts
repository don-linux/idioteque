import { describe, expect, it, vi } from "vitest";
import {
  handleTreeToggleShortcut,
  isTerminalTarget,
  isTreeToggleShortcut,
} from "./panel-shortcuts";

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

describe("isTreeToggleShortcut", () => {
  it("matches only plain Ctrl+B", () => {
    expect(isTreeToggleShortcut(key())).toBe(true);
    expect(isTreeToggleShortcut(key({ code: "KeyJ" }))).toBe(false);
    expect(isTreeToggleShortcut(key({ ctrlKey: false }))).toBe(false);
    expect(isTreeToggleShortcut(key({ metaKey: true }))).toBe(false);
    expect(isTreeToggleShortcut(key({ shiftKey: true }))).toBe(false);
    expect(isTreeToggleShortcut(key({ altKey: true }))).toBe(false);
  });
});

describe("handleTreeToggleShortcut", () => {
  it("toggles the tree and swallows the chord", () => {
    const event = key();
    const toggleTree = vi.fn();

    handleTreeToggleShortcut(event, { hasWorkspace: true, insideTerminal: false, toggleTree });

    expect(toggleTree).toHaveBeenCalledTimes(1);
    expect(event.preventDefault).toHaveBeenCalledTimes(1);
    expect(event.stopPropagation).toHaveBeenCalledTimes(1);
    expect(event.stopImmediatePropagation).toHaveBeenCalledTimes(1);
  });

  it("is a no-op without a workspace", () => {
    const event = key();
    const toggleTree = vi.fn();

    handleTreeToggleShortcut(event, { hasWorkspace: false, insideTerminal: false, toggleTree });

    expect(toggleTree).not.toHaveBeenCalled();
    expect(event.preventDefault).not.toHaveBeenCalled();
  });

  it("leaves Ctrl+B alone inside the terminal, so tmux keeps its prefix", () => {
    const event = key();
    const toggleTree = vi.fn();

    handleTreeToggleShortcut(event, { hasWorkspace: true, insideTerminal: true, toggleTree });

    expect(toggleTree).not.toHaveBeenCalled();
    expect(event.preventDefault).not.toHaveBeenCalled();
    expect(event.stopPropagation).not.toHaveBeenCalled();
  });

  it("ignores other chords without touching the event", () => {
    const event = key({ shiftKey: true });
    const toggleTree = vi.fn();

    handleTreeToggleShortcut(event, { hasWorkspace: true, insideTerminal: false, toggleTree });

    expect(toggleTree).not.toHaveBeenCalled();
    expect(event.preventDefault).not.toHaveBeenCalled();
  });
});

describe("isTerminalTarget", () => {
  it("is false without an element", () => {
    expect(isTerminalTarget(null)).toBe(false);
    expect(isTerminalTarget({} as EventTarget)).toBe(false);
  });

  it("walks up to an xterm ancestor", () => {
    const closest = vi.fn((selector: string) => (selector === ".xterm" ? {} : null));

    expect(isTerminalTarget({ closest } as unknown as EventTarget)).toBe(true);
    expect(closest).toHaveBeenCalledWith(".xterm");
  });

  it("is false for an element outside the terminal", () => {
    const target = { closest: () => null } as unknown as EventTarget;

    expect(isTerminalTarget(target)).toBe(false);
  });

  it("treats an undefined match as outside", () => {
    const target = { closest: () => undefined } as unknown as EventTarget;

    expect(isTerminalTarget(target)).toBe(false);
  });
});
