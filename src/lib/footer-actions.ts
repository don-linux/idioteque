export const FOOTER_ACTION_IDS = ["home", "folder", "settings", "terminal", "git"] as const;

export type FooterActionId = (typeof FOOTER_ACTION_IDS)[number];

export const DEFAULT_FOOTER_ACTION_ORDER: FooterActionId[] = [...FOOTER_ACTION_IDS];

const KNOWN = new Set<string>(FOOTER_ACTION_IDS);

function isFooterActionId(id: string): id is FooterActionId {
  return KNOWN.has(id);
}

export function normalizeFooterOrder(saved: readonly string[] | null | undefined): FooterActionId[] {
  const seen = new Set<FooterActionId>();
  const ordered: FooterActionId[] = [];

  for (const id of saved ?? []) {
    if (!isFooterActionId(id) || seen.has(id)) continue;
    ordered.push(id);
    seen.add(id);
  }

  for (const id of DEFAULT_FOOTER_ACTION_ORDER) {
    if (seen.has(id)) continue;
    ordered.push(id);
  }

  return ordered;
}

export function moveFooterAction(
  order: readonly FooterActionId[],
  from: number,
  to: number,
): FooterActionId[] {
  if (from === to) return [...order];
  if (from < 0 || to < 0 || from >= order.length || to >= order.length) {
    return [...order];
  }

  const next = [...order];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}
