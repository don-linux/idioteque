import { describe, expect, it } from "vitest";
import type { CefUpdateEvent } from "./cef-notices";
import { drainCefUpdates, enqueueCefUpdate, shouldDefer } from "./cef-update";

const updated: CefUpdateEvent = {
  kind: "updated",
  chromium: "153.0.8000.10",
  cef: "153.0.1+gabc",
};

const incompatible: CefUpdateEvent = {
  kind: "incompatible",
  candidateChromium: "153.0.8000.10",
  candidateCef: "153.0.1+gabc",
  currentChromium: "152.0.7977.83",
  currentCef: "152.0.6+gdef",
  reason: "health-exit-10",
};

describe("shouldDefer", () => {
  it("defers only while the browser surface is visible", () => {
    expect(shouldDefer("browser")).toBe(true);
    expect(shouldDefer("editor")).toBe(false);
    expect(shouldDefer("terminals")).toBe(false);
  });
});

describe("enqueueCefUpdate", () => {
  it("appends without mutating the previous list", () => {
    const first = enqueueCefUpdate([], updated);
    const next = enqueueCefUpdate(first, incompatible);

    expect(first).toEqual([updated]);
    expect(next).toEqual([updated, incompatible]);
  });
});

describe("drainCefUpdates", () => {
  it("returns the queued events and an empty remainder", () => {
    const queued = enqueueCefUpdate(enqueueCefUpdate([], updated), incompatible);
    expect(drainCefUpdates(queued)).toEqual({
      pending: [],
      events: [updated, incompatible],
    });
  });

  it("is a no-op on an empty queue", () => {
    expect(drainCefUpdates([])).toEqual({ pending: [], events: [] });
  });
});
