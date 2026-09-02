export const FOOTER_ACTION_IDS = ["home", "folder", "settings", "terminal", "git"] as const;

export type FooterActionId = (typeof FOOTER_ACTION_IDS)[number];

export const DEFAULT_FOOTER_ACTION_ORDER: FooterActionId[] = [...FOOTER_ACTION_IDS];

export type FooterActionIntent = FooterActionId | "idle";

/** Git is a liveness icon only. Click must not navigate or toggle anything. */
export function footerActionIntent(id: FooterActionId): FooterActionIntent {
  if (id === "git") return "idle";
  return id;
}

export function runFooterAction(
  id: FooterActionId,
  actions: {
    home: () => void;
    folder: () => void;
    terminal: () => void;
  },
): void {
  switch (footerActionIntent(id)) {
    case "home":
      actions.home();
      return;
    case "folder":
      actions.folder();
      return;
    case "terminal":
      actions.terminal();
      return;
    case "idle":
    case "settings":
      return;
  }
}
