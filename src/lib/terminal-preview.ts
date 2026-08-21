export const TERMINAL_PREVIEW_CWD = "~/notas";
export const TERMINAL_PREVIEW_GLYPH = "❯";
export const TERMINAL_PREVIEW_ROWS = 2;
export const TERMINAL_PREVIEW_LINE_HEIGHT = 1.35;
export const TERMINAL_PREVIEW_PADDING_Y = 10;

/** Idle two-line prompt. ANSI blue/green so the selected theme paints it. */
export const TERMINAL_PREVIEW_PROMPT =
  `\x1b[34m${TERMINAL_PREVIEW_CWD}\x1b[0m\r\n\x1b[32m${TERMINAL_PREVIEW_GLYPH}\x1b[0m `;

export function terminalPreviewHostHeight(fontSize: number): number {
  return (
    Math.ceil(fontSize * TERMINAL_PREVIEW_LINE_HEIGHT * TERMINAL_PREVIEW_ROWS) +
    TERMINAL_PREVIEW_PADDING_Y
  );
}

export function fitTerminalPreview(
  xterm: { rows: number; cols: number; resize: (cols: number, rows: number) => void },
  fitAddon: { fit: () => void },
  node: { clientWidth: number; clientHeight: number },
): void {
  if (node.clientWidth < 2 || node.clientHeight < 2) return;

  try {
    fitAddon.fit();
  } catch {
    return;
  }

  if (xterm.rows !== TERMINAL_PREVIEW_ROWS) {
    xterm.resize(xterm.cols, TERMINAL_PREVIEW_ROWS);
  }
}
