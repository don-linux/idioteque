export const DEFAULT_UI_THEME_ID = "idioteque-dark" as const;

export const UI_THEME_IDS = [
  "idioteque-dark",
  "tokyo-dark",
  "idioteque-light",
] as const;

export type UiThemeId = (typeof UI_THEME_IDS)[number];

export const UI_THEME_TOKEN_NAMES = [
  "--bg",
  "--surface",
  "--surface-hover",
  "--border",
  "--text",
  "--text-muted",
  "--text-faint",
  "--accent",
  "--accent-soft",
  "--danger",
  "--shadow",
  "--syntax-heading",
  "--syntax-comment",
  "--syntax-link",
  "--syntax-code",
] as const;

export type UiThemeTokenName = (typeof UI_THEME_TOKEN_NAMES)[number];

export type UiThemeTokens = Record<UiThemeTokenName, string>;

export interface UiThemeDefinition {
  id: UiThemeId;
  label: string;
  scheme: "dark" | "light";
  tokens: UiThemeTokens;
}

const IDIOTEQUE_DARK: UiThemeTokens = {
  "--bg": "#1c1e22",
  "--surface": "#24272d",
  "--surface-hover": "#2c3038",
  "--border": "#3a404a",
  "--text": "#d2d5db",
  "--text-muted": "#8f96a1",
  "--text-faint": "#6a7080",
  "--accent": "#7b9ee8",
  "--accent-soft": "#7b9ee822",
  "--danger": "#e08b99",
  "--shadow": "#00000055",
  "--syntax-heading": "#8eb0ee",
  "--syntax-comment": "#6a7080",
  "--syntax-link": "#7b9ee8",
  "--syntax-code": "#c4a882",
};

const TOKYO_DARK: UiThemeTokens = {
  "--bg": "#14161a",
  "--surface": "#191c21",
  "--surface-hover": "#22262d",
  "--border": "#2a2f37",
  "--text": "#e4e6ea",
  "--text-muted": "#9aa1ad",
  "--text-faint": "#666d79",
  "--accent": "#7aa2f7",
  "--accent-soft": "#7aa2f722",
  "--danger": "#f7768e",
  "--shadow": "#00000055",
  "--syntax-heading": "#7aa2f7",
  "--syntax-comment": "#666d79",
  "--syntax-link": "#7aa2f7",
  "--syntax-code": "#e0af68",
};

const IDIOTEQUE_LIGHT: UiThemeTokens = {
  "--bg": "#f2f3f5",
  "--surface": "#f7f8fa",
  "--surface-hover": "#e8eaee",
  "--border": "#d5d8de",
  "--text": "#2c3038",
  "--text-muted": "#5c6370",
  "--text-faint": "#8a909a",
  "--accent": "#3d6ec9",
  "--accent-soft": "#3d6ec91a",
  "--danger": "#c94b61",
  "--shadow": "#1c1e2218",
  "--syntax-heading": "#3d6ec9",
  "--syntax-comment": "#8a909a",
  "--syntax-link": "#3d6ec9",
  "--syntax-code": "#7a5c2e",
};

export const UI_THEMES: readonly UiThemeDefinition[] = [
  { id: "idioteque-dark", label: "Idioteque-dark", scheme: "dark", tokens: IDIOTEQUE_DARK },
  { id: "tokyo-dark", label: "Tokyo-dark", scheme: "dark", tokens: TOKYO_DARK },
  { id: "idioteque-light", label: "Idioteque-light", scheme: "light", tokens: IDIOTEQUE_LIGHT },
];

const THEME_BY_ID = new Map<UiThemeId, UiThemeDefinition>(
  UI_THEMES.map((entry) => [entry.id, entry]),
);

export function isUiThemeId(id: string): id is UiThemeId {
  return THEME_BY_ID.has(id as UiThemeId);
}

export function resolveUiThemeId(id: string | null | undefined): UiThemeId {
  const trimmed = id?.trim();
  if (trimmed && isUiThemeId(trimmed)) return trimmed;
  return DEFAULT_UI_THEME_ID;
}

export function resolveUiTheme(id: string | null | undefined): UiThemeDefinition {
  return THEME_BY_ID.get(resolveUiThemeId(id)) ?? UI_THEMES[0];
}

export function uiThemeLabel(id: string | null | undefined): string {
  return resolveUiTheme(id).label;
}

export function applyTheme(target: HTMLElement, id: string | null | undefined): UiThemeId {
  const resolved = resolveUiThemeId(id);
  const theme = resolveUiTheme(resolved);
  target.dataset.theme = resolved;
  target.style.colorScheme = theme.scheme;

  for (const name of UI_THEME_TOKEN_NAMES) {
    target.style.setProperty(name, theme.tokens[name]);
  }

  return resolved;
}
