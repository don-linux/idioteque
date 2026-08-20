import { describe, expect, it } from "vitest";
import {
  SETTINGS_SAVED_TOAST,
  TOAST_DURATION_MS,
  createSuccessToast,
  withoutToast,
} from "./toast";

describe("createSuccessToast", () => {
  it("builds a success toast with the saved-settings copy", () => {
    expect(createSuccessToast(3, SETTINGS_SAVED_TOAST)).toEqual({
      id: 3,
      message: "Configuración guardada",
      type: "success",
    });
  });
});

describe("withoutToast", () => {
  const first = createSuccessToast(1, "uno");
  const second = createSuccessToast(2, "dos");

  it("removes only the matching toast", () => {
    expect(withoutToast([first, second], 1)).toEqual([second]);
  });

  it("keeps the list when the id is unknown", () => {
    expect(withoutToast([first], 9)).toEqual([first]);
  });
});

describe("TOAST_DURATION_MS", () => {
  it("auto-dismisses after three seconds", () => {
    expect(TOAST_DURATION_MS).toBe(3000);
  });
});
