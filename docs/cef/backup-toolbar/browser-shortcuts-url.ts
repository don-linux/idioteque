/** Snapshot: Ctrl+L focused the IDE URL field. Not imported by the runtime. */

export interface BrowserShortcutEvent {
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  preventDefault: () => void;
  stopPropagation: () => void;
  stopImmediatePropagation?: () => void;
}

interface Chord {
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

export function isBrowserFocusUrlShortcut(event: Chord): boolean {
  return (
    event.code === "KeyL" &&
    event.ctrlKey &&
    !event.metaKey &&
    !event.shiftKey &&
    !event.altKey
  );
}

export function handleBrowserFocusUrlShortcut(
  event: BrowserShortcutEvent,
  ctx: {
    browserSurface: boolean;
    focusUrl: () => void;
  },
): void {
  if (!isBrowserFocusUrlShortcut(event)) return;
  if (!ctx.browserSurface) return;

  event.preventDefault();
  event.stopPropagation();
  event.stopImmediatePropagation?.();
  ctx.focusUrl();
}
