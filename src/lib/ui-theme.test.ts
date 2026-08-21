import { describe, expect, it } from "vitest";
import {
  DEFAULT_UI_THEME_ID,
  UI_THEME_IDS,
  UI_THEME_TOKEN_NAMES,
  UI_THEMES,
  isUiThemeId,
  resolveUiTheme,
  resolveUiThemeId,
  uiThemeLabel,
} from "./ui-theme";

describe("UI_THEMES", () => {
  it("lists Idioteque-dark first as the default", () => {
    expect(UI_THEME_IDS[0]).toBe(DEFAULT_UI_THEME_ID);
    expect(DEFAULT_UI_THEME_ID).toBe("idioteque-dark");
    expect(UI_THEMES.map((theme) => theme.id)).toEqual([
      "idioteque-dark",
      "tokyo-dark",
      "idioteque-light",
    ]);
  });

  it("keeps unique ids and labels", () => {
    const ids = UI_THEMES.map((theme) => theme.id);
    const labels = UI_THEMES.map((theme) => theme.label);
    expect(new Set(ids).size).toBe(ids.length);
    expect(new Set(labels).size).toBe(labels.length);
  });

  it("defines every token on every theme", () => {
    for (const theme of UI_THEMES) {
      for (const name of UI_THEME_TOKEN_NAMES) {
        expect(theme.tokens[name]).toMatch(/^#/);
      }
    }
  });

  it("preserves the previous chrome as Tokyo-dark", () => {
    const tokyo = resolveUiTheme("tokyo-dark");
    expect(tokyo.tokens["--bg"]).toBe("#14161a");
    expect(tokyo.tokens["--accent"]).toBe("#7aa2f7");
    expect(tokyo.tokens["--danger"]).toBe("#f7768e");
    expect(tokyo.scheme).toBe("dark");
  });

  it("uses a lifted original palette for Idioteque-dark", () => {
    const idioteque = resolveUiTheme("idioteque-dark");
    expect(idioteque.tokens["--bg"]).toBe("#1c1e22");
    expect(idioteque.tokens["--text"]).toBe("#d2d5db");
    expect(idioteque.tokens["--accent"]).toBe("#7b9ee8");
    expect(idioteque.scheme).toBe("dark");
  });

  it("uses soft neutrals for Idioteque-light", () => {
    const light = resolveUiTheme("idioteque-light");
    expect(light.tokens["--bg"]).toBe("#f2f3f5");
    expect(light.tokens["--bg"].toLowerCase()).not.toBe("#ffffff");
    expect(light.tokens["--accent"]).toBe("#3d6ec9");
    expect(light.scheme).toBe("light");
  });
});

describe("resolveUiThemeId", () => {
  it("accepts known ids", () => {
    expect(isUiThemeId("tokyo-dark")).toBe(true);
    expect(resolveUiThemeId("tokyo-dark")).toBe("tokyo-dark");
    expect(resolveUiThemeId("  idioteque-light  ")).toBe("idioteque-light");
  });

  it("falls back to Idioteque-dark", () => {
    expect(resolveUiThemeId(null)).toBe("idioteque-dark");
    expect(resolveUiThemeId(undefined)).toBe("idioteque-dark");
    expect(resolveUiThemeId("")).toBe("idioteque-dark");
    expect(resolveUiThemeId("not-a-theme")).toBe("idioteque-dark");
    expect(uiThemeLabel("ghost")).toBe("Idioteque-dark");
  });
});

describe("uiThemeLabel", () => {
  it("returns the catalog label for a known id", () => {
    expect(uiThemeLabel("tokyo-dark")).toBe("Tokyo-dark");
    expect(uiThemeLabel("idioteque-light")).toBe("Idioteque-light");
  });
});
