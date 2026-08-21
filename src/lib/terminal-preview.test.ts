import { describe, expect, it } from "vitest";
import {
  MAX_TERMINAL_FONT_SIZE,
  MIN_TERMINAL_FONT_SIZE,
  TERMINAL_FONT_PREVIEW,
} from "./terminal-font";
import {
  TERMINAL_PREVIEW_BUFFER,
  TERMINAL_PREVIEW_LINE_HEIGHT,
  TERMINAL_PREVIEW_OPTIONS,
  TERMINAL_PREVIEW_PADDING_PX,
  TERMINAL_PREVIEW_ROWS,
  terminalPreviewHostHeight,
} from "./terminal-preview";

describe("terminal preview sample", () => {
  it("reuses the existing prompt line", () => {
    expect(TERMINAL_PREVIEW_BUFFER).toBe(TERMINAL_FONT_PREVIEW);
    expect(TERMINAL_PREVIEW_BUFFER).toContain("git status");
  });

  it("keeps a compact two-row window", () => {
    expect(TERMINAL_PREVIEW_ROWS).toBe(2);
    expect(TERMINAL_PREVIEW_OPTIONS.disableStdin).toBe(true);
    expect(TERMINAL_PREVIEW_OPTIONS.scrollback).toBe(0);
  });
});

describe("terminalPreviewHostHeight", () => {
  it("grows with font size so the prompt is not clipped", () => {
    const small = terminalPreviewHostHeight(MIN_TERMINAL_FONT_SIZE);
    const large = terminalPreviewHostHeight(MAX_TERMINAL_FONT_SIZE);
    expect(small).toBe(
      Math.ceil(MIN_TERMINAL_FONT_SIZE * TERMINAL_PREVIEW_ROWS * TERMINAL_PREVIEW_LINE_HEIGHT) +
        TERMINAL_PREVIEW_PADDING_PX,
    );
    expect(large).toBeGreaterThan(small);
  });

  it("clamps outliers to the terminal font range", () => {
    expect(terminalPreviewHostHeight(3)).toBe(terminalPreviewHostHeight(MIN_TERMINAL_FONT_SIZE));
    expect(terminalPreviewHostHeight(99)).toBe(terminalPreviewHostHeight(MAX_TERMINAL_FONT_SIZE));
    expect(terminalPreviewHostHeight(Number.NaN)).toBe(terminalPreviewHostHeight(13));
  });
});
