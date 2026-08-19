export type TerminalDock = "bottom" | "right";

export function nextDockToggle(
  open: boolean,
  dock: TerminalDock,
  next: TerminalDock,
): { open: boolean; dock: TerminalDock } {
  if (!open) return { open: true, dock: next };
  if (dock === next) return { open: false, dock };
  return { open: true, dock: next };
}

export function dockFromAlt(altKey: boolean): TerminalDock {
  return altKey ? "right" : "bottom";
}

export function isTerminalDockShortcut(event: {
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}): boolean {
  return event.code === "KeyJ" && event.ctrlKey && !event.metaKey && !event.shiftKey;
}

export interface TerminalShortcutEvent {
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  preventDefault: () => void;
  stopPropagation: () => void;
  stopImmediatePropagation?: () => void;
}

export function handleTerminalShortcut(
  event: TerminalShortcutEvent,
  ctx: { hasWorkspace: boolean; toggle: (dock: TerminalDock) => void },
): void {
  if (!isTerminalDockShortcut(event)) return;
  if (!ctx.hasWorkspace) return;

  event.preventDefault();
  event.stopPropagation();
  event.stopImmediatePropagation?.();
  ctx.toggle(dockFromAlt(event.altKey));
}
