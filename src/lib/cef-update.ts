import type { CefUpdateEvent } from "$lib/cef-notices";
import type { WorkspaceSurface } from "$lib/workspace-surface";

export function shouldDefer(surface: WorkspaceSurface): boolean {
  return surface === "browser";
}

export function enqueueCefUpdate(
  pending: readonly CefUpdateEvent[],
  event: CefUpdateEvent,
): CefUpdateEvent[] {
  return [...pending, event];
}

export function drainCefUpdates(pending: readonly CefUpdateEvent[]): {
  pending: CefUpdateEvent[];
  events: CefUpdateEvent[];
} {
  return { pending: [], events: [...pending] };
}
