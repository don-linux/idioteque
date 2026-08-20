import { describe, expect, it } from "vitest";
import { TERMINAL_THEME, TERMINAL_XTERM_OPTIONS } from "./terminal-theme";

const TANGO_BLUE = "#3465a4";
const TANGO_YELLOW = "#c4a000";

const ANSI_SLOTS = [
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
  "brightBlack",
  "brightRed",
  "brightGreen",
  "brightYellow",
  "brightBlue",
  "brightMagenta",
  "brightCyan",
  "brightWhite",
] as const;

describe("TERMINAL_THEME", () => {
  it("keeps the idioteque chrome colors", () => {
    expect(TERMINAL_THEME.background).toBe("#14161a");
    expect(TERMINAL_THEME.foreground).toBe("#e4e6ea");
    expect(TERMINAL_THEME.cursor).toBe("#7aa2f7");
    expect(TERMINAL_THEME.cursorAccent).toBe("#14161a");
    expect(TERMINAL_THEME.selectionBackground).toBe("#7aa2f722");
  });

  it("defines the full 16-color ANSI palette", () => {
    for (const slot of ANSI_SLOTS) {
      expect(TERMINAL_THEME[slot]).toMatch(/^#[0-9a-f]{6}$/);
    }
  });

  it("uses Tokyo Night ANSI colors instead of xterm Tango defaults", () => {
    expect(TERMINAL_THEME.blue).toBe("#7aa2f7");
    expect(TERMINAL_THEME.yellow).toBe("#e0af68");
    expect(TERMINAL_THEME.red).toBe("#f7768e");
    expect(TERMINAL_THEME.green).toBe("#9ece6a");
    expect(TERMINAL_THEME.blue.toLowerCase()).not.toBe(TANGO_BLUE);
    expect(TERMINAL_THEME.yellow.toLowerCase()).not.toBe(TANGO_YELLOW);
  });
});

describe("TERMINAL_XTERM_OPTIONS", () => {
  it("keeps color fidelity and custom Powerline glyphs", () => {
    expect(TERMINAL_XTERM_OPTIONS.theme).toBe(TERMINAL_THEME);
    expect(TERMINAL_XTERM_OPTIONS.customGlyphs).toBe(true);
    expect(TERMINAL_XTERM_OPTIONS.drawBoldTextInBrightColors).toBe(true);
    expect(TERMINAL_XTERM_OPTIONS.minimumContrastRatio).toBe(1);
    expect(TERMINAL_XTERM_OPTIONS.cursorBlink).toBe(true);
  });
});
