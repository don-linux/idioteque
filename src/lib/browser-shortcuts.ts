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

/** Ctrl+B. Not delivered while the terminal has focus (tmux prefix). */
export function isBrowserToggleShortcut(event: Chord): boolean {
  return (
    event.code === "KeyB" &&
    event.ctrlKey &&
    !event.metaKey &&
    !event.shiftKey &&
    !event.altKey
  );
}

/** Ctrl+Shift+B: the one that also works from inside the terminal. */
export function isBrowserToggleAnywhereShortcut(event: Chord): boolean {
  return (
    event.code === "KeyB" &&
    event.ctrlKey &&
    event.shiftKey &&
    !event.metaKey &&
    !event.altKey
  );
}

/** Ctrl+L: focus the URL. X11 stays on the Tauri toplevel, so wry must handle it. */
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

export function handleBrowserShortcut(
  event: BrowserShortcutEvent,
  ctx: {
    hasWorkspace: boolean;
    insideTerminal: boolean;
    toggleBrowser: () => void;
  },
): void {
  const anywhere = isBrowserToggleAnywhereShortcut(event);
  const plain = isBrowserToggleShortcut(event);

  if (!anywhere && !plain) return;
  if (!ctx.hasWorkspace) return;
  // Ctrl+B is the tmux prefix, so a focused terminal keeps it. Ctrl+Shift+B is ours.
  if (plain && ctx.insideTerminal) return;

  event.preventDefault();
  event.stopPropagation();
  event.stopImmediatePropagation?.();
  ctx.toggleBrowser();
}
