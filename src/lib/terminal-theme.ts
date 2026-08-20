import type { ITerminalOptions, ITheme } from "@xterm/xterm";

export const TERMINAL_THEME = {
  background: "#14161a",
  foreground: "#e4e6ea",
  cursor: "#7aa2f7",
  cursorAccent: "#14161a",
  selectionBackground: "#7aa2f722",
  black: "#15161e",
  red: "#f7768e",
  green: "#9ece6a",
  yellow: "#e0af68",
  blue: "#7aa2f7",
  magenta: "#bb9af7",
  cyan: "#7dcfff",
  white: "#a9b1d6",
  brightBlack: "#414868",
  brightRed: "#f7768e",
  brightGreen: "#9ece6a",
  brightYellow: "#e0af68",
  brightBlue: "#7aa2f7",
  brightMagenta: "#bb9af7",
  brightCyan: "#7dcfff",
  brightWhite: "#c0caf5",
} as const satisfies ITheme;

export const TERMINAL_XTERM_OPTIONS = {
  cursorBlink: true,
  customGlyphs: true,
  drawBoldTextInBrightColors: true,
  minimumContrastRatio: 1,
  theme: TERMINAL_THEME,
} as const satisfies Partial<ITerminalOptions>;
