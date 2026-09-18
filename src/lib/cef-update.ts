import type { CefUpdateEvent } from "$lib/cef-notices";

export function shouldDefer(): boolean {
  return false;
}

export function cefUpdateKey(event: CefUpdateEvent): string {
  if (event.kind === "updated") {
    return `updated\0${event.chromium}\0${event.cef}`;
  }
  return [
    "incompatible",
    event.candidateChromium,
    event.candidateCef,
    event.currentChromium,
    event.currentCef,
    event.reason,
  ].join("\0");
}

export function sameCefUpdate(a: CefUpdateEvent, b: CefUpdateEvent): boolean {
  return cefUpdateKey(a) === cefUpdateKey(b);
}

export function enqueueCefUpdate(
  pending: readonly CefUpdateEvent[],
  event: CefUpdateEvent,
): CefUpdateEvent[] {
  if (pending.some((item) => sameCefUpdate(item, event))) {
    return [...pending];
  }
  return [...pending, event];
}

export function drainCefUpdates(pending: readonly CefUpdateEvent[]): {
  pending: CefUpdateEvent[];
  events: CefUpdateEvent[];
} {
  return { pending: [], events: [...pending] };
}
