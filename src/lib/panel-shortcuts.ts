export interface TreeToggleEvent {
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

/** Ctrl+B, like Visual Studio Code. Not delivered while the terminal has focus. */
export function isTreeToggleShortcut(event: Chord): boolean {
  return (
    event.code === "KeyB" &&
    event.ctrlKey &&
    !event.metaKey &&
    !event.shiftKey &&
    !event.altKey
  );
}

/** Ctrl+Shift+B: the one that also works from inside the terminal. */
export function isTreeToggleAnywhereShortcut(event: Chord): boolean {
  return (
    event.code === "KeyB" &&
    event.ctrlKey &&
    event.shiftKey &&
    !event.metaKey &&
    !event.altKey
  );
}

export function handleTreeToggleShortcut(
  event: TreeToggleEvent,
  ctx: {
    hasWorkspace: boolean;
    insideTerminal: boolean;
    toggleTree: () => void;
  },
): void {
  const anywhere = isTreeToggleAnywhereShortcut(event);
  const plain = isTreeToggleShortcut(event);

  if (!anywhere && !plain) return;
  if (!ctx.hasWorkspace) return;
  // Ctrl+B is the tmux prefix, so a focused terminal keeps it. Ctrl+Shift+B is ours.
  if (plain && ctx.insideTerminal) return;

  event.preventDefault();
  event.stopPropagation();
  event.stopImmediatePropagation?.();
  ctx.toggleTree();
}

/**
 * True when the event came from inside an xterm surface. Duck typed on `closest`
 * instead of `instanceof Element`, which also covers nodes from another document.
 */
export function isTerminalTarget(target: EventTarget | null): boolean {
  const candidate = target as { closest?: (selector: string) => unknown } | null;
  if (!candidate || typeof candidate.closest !== "function") return false;
  return candidate.closest(".xterm") != null;
}
