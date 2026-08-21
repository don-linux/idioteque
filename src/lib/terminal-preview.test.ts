import { describe, expect, it } from "vitest";
import {
  TERMINAL_PREVIEW_CWD,
  TERMINAL_PREVIEW_GLYPH,
  TERMINAL_PREVIEW_PADDING_Y,
  TERMINAL_PREVIEW_PROMPT,
  TERMINAL_PREVIEW_ROWS,
  fitTerminalPreview,
  terminalPreviewHostHeight,
} from "./terminal-preview";

describe("terminal preview prompt", () => {
  it("is an idle two-line prompt with ANSI path and glyph colors", () => {
    expect(TERMINAL_PREVIEW_PROMPT).toContain(TERMINAL_PREVIEW_CWD);
    expect(TERMINAL_PREVIEW_PROMPT).toContain(TERMINAL_PREVIEW_GLYPH);
    expect(TERMINAL_PREVIEW_PROMPT).toContain("\x1b[34m");
    expect(TERMINAL_PREVIEW_PROMPT).toContain("\x1b[32m");
    expect(TERMINAL_PREVIEW_PROMPT).toContain("\x1b[0m");
    expect(TERMINAL_PREVIEW_PROMPT).toContain("\r\n");
  });

  it("does not look like a command already typed", () => {
    expect(TERMINAL_PREVIEW_PROMPT.toLowerCase()).not.toContain("git");
    expect(TERMINAL_PREVIEW_PROMPT.toLowerCase()).not.toContain("status");
  });
});

describe("terminalPreviewHostHeight", () => {
  it("grows with font size and fits two rows", () => {
    const small = terminalPreviewHostHeight(10);
    const large = terminalPreviewHostHeight(24);

    expect(large).toBeGreaterThan(small);
    expect(small).toBeGreaterThanOrEqual(10 * TERMINAL_PREVIEW_ROWS + TERMINAL_PREVIEW_PADDING_Y);
    expect(large).toBeGreaterThanOrEqual(24 * TERMINAL_PREVIEW_ROWS + TERMINAL_PREVIEW_PADDING_Y);
  });
});

describe("fitTerminalPreview", () => {
  it("does nothing when the host is not laid out", () => {
    const resize = () => {
      throw new Error("should not resize");
    };
    const fit = () => {
      throw new Error("should not fit");
    };

    expect(() =>
      fitTerminalPreview({ rows: 2, cols: 40, resize }, { fit }, { clientWidth: 0, clientHeight: 0 }),
    ).not.toThrow();
  });

  it("fits and clamps rows to the compact preview size", () => {
    let fitted = false;
    let next = { cols: 80, rows: 24 };

    fitTerminalPreview(
      {
        get cols() {
          return next.cols;
        },
        get rows() {
          return next.rows;
        },
        resize(cols, rows) {
          next = { cols, rows };
        },
      },
      {
        fit() {
          fitted = true;
          next = { cols: 72, rows: 8 };
        },
      },
      { clientWidth: 480, clientHeight: 40 },
    );

    expect(fitted).toBe(true);
    expect(next).toEqual({ cols: 72, rows: TERMINAL_PREVIEW_ROWS });
  });
});
