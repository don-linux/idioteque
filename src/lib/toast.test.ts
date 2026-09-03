import { describe, expect, it } from "vitest";
import {
  HINT_TOAST_DURATION_MS,
  SETTINGS_SAVED_TOAST,
  TOAST_DURATION_MS,
  createHintToast,
  createSuccessToast,
  toastsForPlacement,
  withoutToast,
} from "./toast";

describe("createSuccessToast", () => {
  it("builds a success toast with the saved-settings copy", () => {
    expect(createSuccessToast(3, SETTINGS_SAVED_TOAST)).toEqual({
      id: 3,
      message: "Configuración guardada",
      type: "success",
      placement: "bottom-right",
    });
  });
});

describe("createHintToast", () => {
  it("places hints at the top right", () => {
    expect(createHintToast(4, "hint")).toEqual({
      id: 4,
      message: "hint",
      type: "hint",
      placement: "top-right",
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

describe("toastsForPlacement", () => {
  const success = createSuccessToast(1, "abajo");
  const hint = createHintToast(2, "arriba");

  it("splits hosts so settings stay at the bottom", () => {
    expect(toastsForPlacement([success, hint], "bottom-right")).toEqual([success]);
    expect(toastsForPlacement([success, hint], "top-right")).toEqual([hint]);
  });
});

describe("durations", () => {
  it("auto-dismisses success after three seconds", () => {
    expect(TOAST_DURATION_MS).toBe(3000);
  });

  it("keeps hints visible longer", () => {
    expect(HINT_TOAST_DURATION_MS).toBe(8000);
  });
});
