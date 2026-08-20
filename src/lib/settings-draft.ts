export interface TerminalDraft {
  fontFamily: string | null;
  fontSize: number;
}

export function normalizeFontFamily(family: string | null | undefined): string | null {
  const trimmed = family?.trim();
  return trimmed ? trimmed : null;
}

export function isSettingsDirty(draft: TerminalDraft, saved: TerminalDraft): boolean {
  return draft.fontFamily !== saved.fontFamily || draft.fontSize !== saved.fontSize;
}
