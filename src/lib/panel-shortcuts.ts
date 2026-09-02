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

export function isTreeToggleShortcut(event: {
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}): boolean {
  return (
    event.code === "KeyB" &&
    event.ctrlKey &&
    !event.metaKey &&
    !event.shiftKey &&
    !event.altKey
  );
}

export function handleTreeToggleShortcut(
  event: TreeToggleEvent,
  ctx: {
    hasWorkspace: boolean;
    /// Ctrl+B is the tmux prefix, so the terminal keeps it when it has focus.
    insideTerminal: boolean;
    toggleTree: () => void;
  },
): void {
  if (!isTreeToggleShortcut(event)) return;
  if (!ctx.hasWorkspace || ctx.insideTerminal) return;

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
