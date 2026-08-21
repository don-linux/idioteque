import type { ITerminalOptions } from "@xterm/xterm";
import { clampFontSize, TERMINAL_FONT_PREVIEW } from "./terminal-font";

export const TERMINAL_PREVIEW_BUFFER = TERMINAL_FONT_PREVIEW;
export const TERMINAL_PREVIEW_ROWS = 2;
export const TERMINAL_PREVIEW_LINE_HEIGHT = 1.25;
export const TERMINAL_PREVIEW_PADDING_PX = 10;

export const TERMINAL_PREVIEW_OPTIONS = {
  disableStdin: true,
  scrollback: 0,
  cursorInactiveStyle: "bar",
} as const satisfies Partial<ITerminalOptions>;

export function terminalPreviewHostHeight(fontSize: number): number {
  const size = clampFontSize(fontSize);
  return (
    Math.ceil(size * TERMINAL_PREVIEW_ROWS * TERMINAL_PREVIEW_LINE_HEIGHT) +
    TERMINAL_PREVIEW_PADDING_PX
  );
}
