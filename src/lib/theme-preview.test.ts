import { describe, expect, it } from "vitest";
import {
  THEME_PREVIEW_DIR,
  THEME_PREVIEW_FILES,
  THEME_PREVIEW_MARKDOWN,
  THEME_PREVIEW_ROOT,
  THEME_PREVIEW_STATUS,
} from "./theme-preview";

describe("theme preview sample", () => {
  it("includes a heading, list, link, and inline code", () => {
    expect(THEME_PREVIEW_MARKDOWN).toContain("# Guía rápida");
    expect(THEME_PREVIEW_MARKDOWN).toContain("- ");
    expect(THEME_PREVIEW_MARKDOWN).toContain("[enlace]");
    expect(THEME_PREVIEW_MARKDOWN).toContain("`const hola");
    expect(THEME_PREVIEW_MARKDOWN).toContain("<!-- comentario");
  });

  it("describes a small static workspace chrome", () => {
    expect(THEME_PREVIEW_ROOT).toBe("notas");
    expect(THEME_PREVIEW_DIR).toBe("diario");
    expect(THEME_PREVIEW_STATUS).toBe("guardado");
    expect(THEME_PREVIEW_FILES.some((file) => file.selected)).toBe(true);
  });
});
