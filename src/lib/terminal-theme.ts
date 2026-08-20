import type { ITerminalOptions, ITheme } from "@xterm/xterm";

export const DEFAULT_TERMINAL_THEME_ID = "tokyo-night" as const;

export const TERMINAL_THEME_IDS = [
  "tokyo-night",
  "dracula",
  "nord",
  "gruvbox-dark",
  "catppuccin-mocha",
  "one-half-dark",
  "solarized-dark",
  "campbell",
] as const;

export type TerminalThemeId = (typeof TERMINAL_THEME_IDS)[number];

export const TERMINAL_ANSI_SLOTS = [
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

export type TerminalAnsiSlot = (typeof TERMINAL_ANSI_SLOTS)[number];

export interface TerminalThemeDefinition {
  id: TerminalThemeId;
  label: string;
  theme: ITheme;
}

/**
 * Tokyo Night Night from folke/tokyonight.nvim extras (Alacritty/WezTerm/Ghostty).
 * Ghostty ships this via iTerm2-Color-Schemes, which credits folke.
 */
const TOKYO_NIGHT: ITheme = {
  background: "#1a1b26",
  foreground: "#c0caf5",
  cursor: "#c0caf5",
  cursorAccent: "#1a1b26",
  selectionBackground: "#283457",
  selectionForeground: "#c0caf5",
  black: "#15161e",
  red: "#f7768e",
  green: "#9ece6a",
  yellow: "#e0af68",
  blue: "#7aa2f7",
  magenta: "#bb9af7",
  cyan: "#7dcfff",
  white: "#a9b1d6",
  brightBlack: "#414868",
  brightRed: "#ff899d",
  brightGreen: "#9fe044",
  brightYellow: "#faba4a",
  brightBlue: "#8db0ff",
  brightMagenta: "#c7a9ff",
  brightCyan: "#a4daff",
  brightWhite: "#c0caf5",
};

/** Dracula Syntax Highlighting Specification §1.2 */
const DRACULA: ITheme = {
  background: "#282a36",
  foreground: "#f8f8f2",
  cursor: "#f8f8f2",
  cursorAccent: "#282a36",
  selectionBackground: "#44475a",
  black: "#21222c",
  red: "#ff5555",
  green: "#50fa7b",
  yellow: "#f1fa8c",
  blue: "#bd93f9",
  magenta: "#ff79c6",
  cyan: "#8be9fd",
  white: "#f8f8f2",
  brightBlack: "#6272a4",
  brightRed: "#ff6e6e",
  brightGreen: "#69ff94",
  brightYellow: "#ffffa5",
  brightBlue: "#d6acff",
  brightMagenta: "#ff92df",
  brightCyan: "#a4ffff",
  brightWhite: "#ffffff",
};

/**
 * Nord official palette (nord0–nord15) mapped the terminal way:
 * bg nord0, fg nord4, selection nord2, ANSI aurora/frost.
 * https://www.nordtheme.com/docs/colors-and-palettes
 */
const NORD: ITheme = {
  background: "#2e3440",
  foreground: "#d8dee9",
  cursor: "#d8dee9",
  cursorAccent: "#2e3440",
  selectionBackground: "#434c5e",
  black: "#3b4252",
  red: "#bf616a",
  green: "#a3be8c",
  yellow: "#ebcb8b",
  blue: "#81a1c1",
  magenta: "#b48ead",
  cyan: "#88c0d0",
  white: "#e5e9f0",
  brightBlack: "#4c566a",
  brightRed: "#bf616a",
  brightGreen: "#a3be8c",
  brightYellow: "#ebcb8b",
  brightBlue: "#81a1c1",
  brightMagenta: "#b48ead",
  brightCyan: "#8fbcbb",
  brightWhite: "#eceff4",
};

/** morhetz/gruvbox dark0 + neutral/bright ANSI */
const GRUVBOX_DARK: ITheme = {
  background: "#282828",
  foreground: "#ebdbb2",
  cursor: "#ebdbb2",
  cursorAccent: "#282828",
  selectionBackground: "#504945",
  black: "#282828",
  red: "#cc241d",
  green: "#98971a",
  yellow: "#d79921",
  blue: "#458588",
  magenta: "#b16286",
  cyan: "#689d6a",
  white: "#a89984",
  brightBlack: "#928374",
  brightRed: "#fb4934",
  brightGreen: "#b8bb26",
  brightYellow: "#fabd2f",
  brightBlue: "#83a598",
  brightMagenta: "#d3869b",
  brightCyan: "#8ec07c",
  brightWhite: "#ebdbb2",
};

/** catppuccin/ghostty themes/catppuccin-mocha.conf */
const CATPPUCCIN_MOCHA: ITheme = {
  background: "#1e1e2e",
  foreground: "#cdd6f4",
  cursor: "#f5e0dc",
  cursorAccent: "#11111b",
  selectionBackground: "#353749",
  selectionForeground: "#cdd6f4",
  black: "#45475a",
  red: "#f38ba8",
  green: "#a6e3a1",
  yellow: "#f9e2af",
  blue: "#89b4fa",
  magenta: "#f5c2e7",
  cyan: "#94e2d5",
  white: "#a6adc8",
  brightBlack: "#585b70",
  brightRed: "#f38ba8",
  brightGreen: "#a6e3a1",
  brightYellow: "#f9e2af",
  brightBlue: "#89b4fa",
  brightMagenta: "#f5c2e7",
  brightCyan: "#94e2d5",
  brightWhite: "#bac2de",
};

/** Windows Terminal built-in "One Half Dark" (defaults.json). purple → magenta. */
const ONE_HALF_DARK: ITheme = {
  background: "#282c34",
  foreground: "#dcdfe4",
  cursor: "#ffffff",
  cursorAccent: "#282c34",
  black: "#282c34",
  red: "#e06c75",
  green: "#98c379",
  yellow: "#e5c07b",
  blue: "#61afef",
  magenta: "#c678dd",
  cyan: "#56b6c2",
  white: "#dcdfe4",
  brightBlack: "#5a6374",
  brightRed: "#e06c75",
  brightGreen: "#98c379",
  brightYellow: "#e5c07b",
  brightBlue: "#61afef",
  brightMagenta: "#c678dd",
  brightCyan: "#56b6c2",
  brightWhite: "#dcdfe4",
};

/** Windows Terminal built-in "Solarized Dark" (defaults.json). purple → magenta. */
const SOLARIZED_DARK: ITheme = {
  background: "#002b36",
  foreground: "#839496",
  cursor: "#ffffff",
  cursorAccent: "#002b36",
  black: "#002b36",
  red: "#dc322f",
  green: "#859900",
  yellow: "#b58900",
  blue: "#268bd2",
  magenta: "#d33682",
  cyan: "#2aa198",
  white: "#eee8d5",
  brightBlack: "#073642",
  brightRed: "#cb4b16",
  brightGreen: "#586e75",
  brightYellow: "#657b83",
  brightBlue: "#839496",
  brightMagenta: "#6c71c4",
  brightCyan: "#93a1a1",
  brightWhite: "#fdf6e3",
};

/** Windows Terminal built-in "Campbell" (defaults.json). purple → magenta. */
const CAMPBELL: ITheme = {
  background: "#0c0c0c",
  foreground: "#cccccc",
  cursor: "#ffffff",
  cursorAccent: "#0c0c0c",
  black: "#0c0c0c",
  red: "#c50f1f",
  green: "#13a10e",
  yellow: "#c19c00",
  blue: "#0037da",
  magenta: "#881798",
  cyan: "#3a96dd",
  white: "#cccccc",
  brightBlack: "#767676",
  brightRed: "#e74856",
  brightGreen: "#16c60c",
  brightYellow: "#f9f1a5",
  brightBlue: "#3b78ff",
  brightMagenta: "#b4009e",
  brightCyan: "#61d6d6",
  brightWhite: "#f2f2f2",
};

export const TERMINAL_THEMES: readonly TerminalThemeDefinition[] = [
  { id: "tokyo-night", label: "Tokyo Night", theme: TOKYO_NIGHT },
  { id: "dracula", label: "Dracula", theme: DRACULA },
  { id: "nord", label: "Nord", theme: NORD },
  { id: "gruvbox-dark", label: "Gruvbox Dark", theme: GRUVBOX_DARK },
  { id: "catppuccin-mocha", label: "Catppuccin Mocha", theme: CATPPUCCIN_MOCHA },
  { id: "one-half-dark", label: "One Half Dark", theme: ONE_HALF_DARK },
  { id: "solarized-dark", label: "Solarized Dark", theme: SOLARIZED_DARK },
  { id: "campbell", label: "Campbell", theme: CAMPBELL },
];

const THEME_BY_ID = new Map<TerminalThemeId, TerminalThemeDefinition>(
  TERMINAL_THEMES.map((entry) => [entry.id, entry]),
);

export function isTerminalThemeId(id: string): id is TerminalThemeId {
  return THEME_BY_ID.has(id as TerminalThemeId);
}

export function resolveTerminalThemeId(id: string | null | undefined): TerminalThemeId {
  const trimmed = id?.trim();
  if (trimmed && isTerminalThemeId(trimmed)) return trimmed;
  return DEFAULT_TERMINAL_THEME_ID;
}

export function resolveTerminalTheme(id: string | null | undefined): ITheme {
  return THEME_BY_ID.get(resolveTerminalThemeId(id))?.theme ?? TOKYO_NIGHT;
}

export function terminalThemeLabel(id: string | null | undefined): string {
  return THEME_BY_ID.get(resolveTerminalThemeId(id))?.label ?? "Tokyo Night";
}

/** Default Night palette. Not the old idioteque chrome hybrid. */
export const TERMINAL_THEME = TOKYO_NIGHT;

export const TERMINAL_XTERM_OPTIONS = {
  cursorBlink: true,
  customGlyphs: true,
  drawBoldTextInBrightColors: true,
  minimumContrastRatio: 1,
} as const satisfies Partial<ITerminalOptions>;
