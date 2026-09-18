import { describe, expect, it } from "vitest";
import type { CefUpdateEvent } from "./cef-notices";
import {
  cefUpdateKey,
  drainCefUpdates,
  enqueueCefUpdate,
  sameCefUpdate,
  shouldDefer,
} from "./cef-update";

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
  it("never defers: there is no internal browser surface", () => {
    expect(shouldDefer()).toBe(false);
  });
});

describe("sameCefUpdate / cefUpdateKey", () => {
  it("matches updated payloads by chromium and cef, not object identity", () => {
    expect(sameCefUpdate(updated, { ...updated })).toBe(true);
    expect(sameCefUpdate(updated, { ...updated, cef: "153.0.2+gxyz" })).toBe(false);
    expect(cefUpdateKey(updated)).not.toBe(cefUpdateKey(incompatible));
  });

  it("treats a different denylist reason as a different notice", () => {
    expect(
      sameCefUpdate(incompatible, { ...incompatible, reason: "health-timeout" }),
    ).toBe(false);
  });
});

describe("enqueueCefUpdate", () => {
  it("appends without mutating the previous list", () => {
    const first = enqueueCefUpdate([], updated);
    const next = enqueueCefUpdate(first, incompatible);

    expect(first).toEqual([updated]);
    expect(next).toEqual([updated, incompatible]);
  });

  it("does not queue a second copy of the same payload", () => {
    const once = enqueueCefUpdate([], updated);
    const twice = enqueueCefUpdate(once, { ...updated });

    expect(once).toEqual([updated]);
    expect(twice).toEqual([updated]);
    expect(twice).not.toBe(once);
  });

  it("still queues an incompatible notice after an updated toast", () => {
    const queued = enqueueCefUpdate(enqueueCefUpdate([], updated), incompatible);
    expect(queued).toHaveLength(2);
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

  it("does not mutate the input list", () => {
    const queued = [updated, incompatible];
    const drained = drainCefUpdates(queued);

    expect(queued).toEqual([updated, incompatible]);
    expect(drained.events).not.toBe(queued);
    expect(drained.pending).not.toBe(queued);
  });

  it("lets a new event land on the remainder after a flush snapshot", () => {
    const snapshot = enqueueCefUpdate([], updated);
    const drained = drainCefUpdates(snapshot);
    const arrivedDuringFlush = enqueueCefUpdate(drained.pending, incompatible);

    expect(drained.events).toEqual([updated]);
    expect(arrivedDuringFlush).toEqual([incompatible]);
  });
});
