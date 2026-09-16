import { describe, expect, it } from "vitest";
import { TERMINAL_THEMES } from "./terminal-theme";
import { UI_THEMES } from "./ui-theme";

describe("theme catalog parity", () => {
  it("offers the same ids in UI and terminal, independently selectable", () => {
    expect(UI_THEMES.map((theme) => theme.id)).toEqual(TERMINAL_THEMES.map((theme) => theme.id));
  });

  it("keeps the same labels for those ids", () => {
    expect(UI_THEMES.map((theme) => theme.label)).toEqual(
      TERMINAL_THEMES.map((theme) => theme.label),
    );
  });
});
