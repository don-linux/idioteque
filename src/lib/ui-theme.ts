export const DEFAULT_UI_THEME_ID = "idioteque-dark" as const;

export const UI_THEME_IDS = [
  "idioteque-dark",
  "tokyo-dark",
  "idioteque-light",
  "platzi",
  "tokyo-night",
  "catppuccin-mocha",
  "nord",
  "gruvbox-dark",
  "everforest-dark",
  "one-dark",
  "solarized-dark",
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
  "--syntax-keyword",
  "--syntax-string",
  "--syntax-number",
  "--syntax-function",
  "--syntax-type",
  "--syntax-variable",
  "--syntax-operator",
  "--syntax-tag",
  "--syntax-invalid",
] as const;

export type UiThemeTokenName = (typeof UI_THEME_TOKEN_NAMES)[number];

export type UiThemeTokens = Record<UiThemeTokenName, string>;

export interface UiThemeDefinition {
  id: UiThemeId;
  label: string;
  scheme: "dark" | "light";
  tokens: UiThemeTokens;
}

function darkTokens(
  tokens: Omit<UiThemeTokens, "--accent-soft" | "--shadow"> &
    Partial<Pick<UiThemeTokens, "--accent-soft" | "--shadow">>,
): UiThemeTokens {
  return {
    "--accent-soft": `${tokens["--accent"]}22`,
    "--shadow": "#00000055",
    ...tokens,
  };
}

/** Product default. Original lifted palette, not a third-party port. */
const IDIOTEQUE_DARK: UiThemeTokens = darkTokens({
  "--bg": "#1c1e22",
  "--surface": "#24272d",
  "--surface-hover": "#2c3038",
  "--border": "#3a404a",
  "--text": "#d2d5db",
  "--text-muted": "#8f96a1",
  "--text-faint": "#6a7080",
  "--accent": "#7b9ee8",
  "--danger": "#e08b99",
  "--syntax-heading": "#8eb0ee",
  "--syntax-comment": "#6a7080",
  "--syntax-link": "#7b9ee8",
  "--syntax-code": "#c4a882",
  "--syntax-keyword": "#7b9ee8",
  "--syntax-string": "#c4a882",
  "--syntax-number": "#e08b99",
  "--syntax-function": "#8eb0ee",
  "--syntax-type": "#c4a882",
  "--syntax-variable": "#d2d5db",
  "--syntax-operator": "#8f96a1",
  "--syntax-tag": "#e08b99",
  "--syntax-invalid": "#e08b99",
});

/** Previous app chrome, kept as a product theme. */
const TOKYO_DARK: UiThemeTokens = darkTokens({
  "--bg": "#14161a",
  "--surface": "#191c21",
  "--surface-hover": "#22262d",
  "--border": "#2a2f37",
  "--text": "#e4e6ea",
  "--text-muted": "#9aa1ad",
  "--text-faint": "#666d79",
  "--accent": "#7aa2f7",
  "--danger": "#f7768e",
  "--syntax-heading": "#7aa2f7",
  "--syntax-comment": "#666d79",
  "--syntax-link": "#7aa2f7",
  "--syntax-code": "#e0af68",
  "--syntax-keyword": "#7aa2f7",
  "--syntax-string": "#e0af68",
  "--syntax-number": "#f7768e",
  "--syntax-function": "#7aa2f7",
  "--syntax-type": "#e0af68",
  "--syntax-variable": "#e4e6ea",
  "--syntax-operator": "#9aa1ad",
  "--syntax-tag": "#f7768e",
  "--syntax-invalid": "#f7768e",
});

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
  "--syntax-keyword": "#3d6ec9",
  "--syntax-string": "#7a5c2e",
  "--syntax-number": "#c94b61",
  "--syntax-function": "#3d6ec9",
  "--syntax-type": "#7a5c2e",
  "--syntax-variable": "#2c3038",
  "--syntax-operator": "#5c6370",
  "--syntax-tag": "#c94b61",
  "--syntax-invalid": "#c94b61",
};

/**
 * Platzi Theme Green Mode from platzi/platzi-theme
 * (marketplace codevars.platzi-theme-for-vs-code).
 * Chrome: themes/Platzi Theme-color-theme.json `colors`.
 * Syntax: the same file `tokenColors` (identical in Classic).
 */
const PLATZI: UiThemeTokens = darkTokens({
  "--bg": "#03091E",
  "--surface": "#090f24",
  "--surface-hover": "#0C1633",
  "--border": "#121F3D",
  "--text": "#eeffff",
  "--text-muted": "#637B9D",
  "--text-faint": "#546E7A",
  "--accent": "#adeb42",
  "--danger": "#FF5370",
  "--syntax-heading": "#C3E88D",
  "--syntax-comment": "#546E7A",
  "--syntax-link": "#82AAFF",
  "--syntax-code": "#C792EA",
  "--syntax-keyword": "#C792EA",
  "--syntax-string": "#C3E88D",
  "--syntax-number": "#F78C6C",
  "--syntax-function": "#82AAFF",
  "--syntax-type": "#FFCB6B",
  "--syntax-variable": "#EEFFFF",
  "--syntax-operator": "#89DDFF",
  "--syntax-tag": "#f07178",
  "--syntax-invalid": "#FF5370",
});

/**
 * Tokyo Night (enkia/tokyo-night-vscode-theme)
 * themes/tokyo-night-color-theme.json
 */
const TOKYO_NIGHT: UiThemeTokens = darkTokens({
  "--bg": "#1a1b26",
  "--surface": "#16161e",
  "--surface-hover": "#202330",
  "--border": "#363b54",
  "--text": "#c0caf5",
  "--text-muted": "#787c99",
  "--text-faint": "#545c7e",
  "--accent": "#7aa2f7",
  "--danger": "#f7768e",
  "--syntax-heading": "#7aa2f7",
  "--syntax-comment": "#51597d",
  "--syntax-link": "#7aa2f7",
  "--syntax-code": "#9ece6a",
  "--syntax-keyword": "#bb9af7",
  "--syntax-string": "#9ece6a",
  "--syntax-number": "#ff9e64",
  "--syntax-function": "#7aa2f7",
  "--syntax-type": "#0db9d7",
  "--syntax-variable": "#c0caf5",
  "--syntax-operator": "#89ddff",
  "--syntax-tag": "#f7768e",
  "--syntax-invalid": "#ff5370",
});

/**
 * Catppuccin Mocha official palette + editor style guide.
 * https://catppuccin.com/palette/
 * https://github.com/catppuccin/catppuccin/blob/main/docs/style-guide.md
 */
const CATPPUCCIN_MOCHA: UiThemeTokens = darkTokens({
  "--bg": "#1e1e2e",
  "--surface": "#181825",
  "--surface-hover": "#313244",
  "--border": "#45475a",
  "--text": "#cdd6f4",
  "--text-muted": "#a6adc8",
  "--text-faint": "#6c7086",
  "--accent": "#89b4fa",
  "--danger": "#f38ba8",
  "--syntax-heading": "#cba6f7",
  "--syntax-comment": "#9399b2",
  "--syntax-link": "#89b4fa",
  "--syntax-code": "#a6e3a1",
  "--syntax-keyword": "#cba6f7",
  "--syntax-string": "#a6e3a1",
  "--syntax-number": "#fab387",
  "--syntax-function": "#89b4fa",
  "--syntax-type": "#f9e2af",
  "--syntax-variable": "#cdd6f4",
  "--syntax-operator": "#89dceb",
  "--syntax-tag": "#f38ba8",
  "--syntax-invalid": "#f38ba8",
});

/**
 * Nord official palettes nord0–nord15.
 * https://www.nordtheme.com/docs/colors-and-palettes
 */
const NORD: UiThemeTokens = darkTokens({
  "--bg": "#2e3440",
  "--surface": "#3b4252",
  "--surface-hover": "#434c5e",
  "--border": "#4c566a",
  "--text": "#eceff4",
  "--text-muted": "#d8dee9",
  "--text-faint": "#4c566a",
  "--accent": "#88c0d0",
  "--danger": "#bf616a",
  "--syntax-heading": "#88c0d0",
  "--syntax-comment": "#4c566a",
  "--syntax-link": "#88c0d0",
  "--syntax-code": "#a3be8c",
  "--syntax-keyword": "#81a1c1",
  "--syntax-string": "#a3be8c",
  "--syntax-number": "#b48ead",
  "--syntax-function": "#88c0d0",
  "--syntax-type": "#8fbcbb",
  "--syntax-variable": "#d8dee9",
  "--syntax-operator": "#81a1c1",
  "--syntax-tag": "#81a1c1",
  "--syntax-invalid": "#bf616a",
});

/**
 * morhetz/gruvbox dark medium (dark0 + bright accents).
 * https://github.com/morhetz/gruvbox
 */
const GRUVBOX_DARK: UiThemeTokens = darkTokens({
  "--bg": "#282828",
  "--surface": "#3c3836",
  "--surface-hover": "#504945",
  "--border": "#665c54",
  "--text": "#ebdbb2",
  "--text-muted": "#a89984",
  "--text-faint": "#928374",
  "--accent": "#83a598",
  "--danger": "#fb4934",
  "--syntax-heading": "#b8bb26",
  "--syntax-comment": "#928374",
  "--syntax-link": "#83a598",
  "--syntax-code": "#b8bb26",
  "--syntax-keyword": "#fb4934",
  "--syntax-string": "#b8bb26",
  "--syntax-number": "#d3869b",
  "--syntax-function": "#b8bb26",
  "--syntax-type": "#fabd2f",
  "--syntax-variable": "#83a598",
  "--syntax-operator": "#fe8019",
  "--syntax-tag": "#fe8019",
  "--syntax-invalid": "#fb4934",
});

/**
 * Everforest Dark Medium from sainnhe/everforest palette.md.
 * https://github.com/sainnhe/everforest/blob/master/palette.md
 */
const EVERFOREST_DARK: UiThemeTokens = darkTokens({
  "--bg": "#2D353B",
  "--surface": "#343F44",
  "--surface-hover": "#3D484D",
  "--border": "#4F585E",
  "--text": "#D3C6AA",
  "--text-muted": "#9DA9A0",
  "--text-faint": "#7A8478",
  "--accent": "#A7C080",
  "--danger": "#E67E80",
  "--syntax-heading": "#E69875",
  "--syntax-comment": "#859289",
  "--syntax-link": "#7FBBB3",
  "--syntax-code": "#A7C080",
  "--syntax-keyword": "#E67E80",
  "--syntax-string": "#A7C080",
  "--syntax-number": "#D699B6",
  "--syntax-function": "#A7C080",
  "--syntax-type": "#DBBC7F",
  "--syntax-variable": "#D3C6AA",
  "--syntax-operator": "#E69875",
  "--syntax-tag": "#E69875",
  "--syntax-invalid": "#E67E80",
});

/**
 * Atom One Dark Syntax colors.less (HSL compiled to the published HEX).
 * https://github.com/atom/atom/blob/master/packages/one-dark-syntax/styles/colors.less
 * UI chrome @base-background-color from one-dark-ui: #21252b.
 */
const ONE_DARK: UiThemeTokens = darkTokens({
  "--bg": "#282c34",
  "--surface": "#21252b",
  "--surface-hover": "#3e4451",
  "--border": "#5c6370",
  "--text": "#abb2bf",
  "--text-muted": "#828997",
  "--text-faint": "#5c6370",
  "--accent": "#61afef",
  "--danger": "#e06c75",
  "--syntax-heading": "#e06c75",
  "--syntax-comment": "#5c6370",
  "--syntax-link": "#61afef",
  "--syntax-code": "#98c379",
  "--syntax-keyword": "#c678dd",
  "--syntax-string": "#98c379",
  "--syntax-number": "#d19a66",
  "--syntax-function": "#61afef",
  "--syntax-type": "#e5c07b",
  "--syntax-variable": "#abb2bf",
  "--syntax-operator": "#56b6c2",
  "--syntax-tag": "#e06c75",
  "--syntax-invalid": "#e06c75",
});

/**
 * Solarized Dark, Ethan Schoonover.
 * https://ethanschoonover.com/solarized/
 */
const SOLARIZED_DARK: UiThemeTokens = darkTokens({
  "--bg": "#002b36",
  "--surface": "#073642",
  "--surface-hover": "#073642",
  "--border": "#586e75",
  "--text": "#839496",
  "--text-muted": "#93a1a1",
  "--text-faint": "#586e75",
  "--accent": "#268bd2",
  "--danger": "#dc322f",
  "--syntax-heading": "#cb4b16",
  "--syntax-comment": "#586e75",
  "--syntax-link": "#268bd2",
  "--syntax-code": "#2aa198",
  "--syntax-keyword": "#859900",
  "--syntax-string": "#2aa198",
  "--syntax-number": "#d33682",
  "--syntax-function": "#268bd2",
  "--syntax-type": "#b58900",
  "--syntax-variable": "#839496",
  "--syntax-operator": "#859900",
  "--syntax-tag": "#268bd2",
  "--syntax-invalid": "#dc322f",
});

export const UI_THEMES: readonly UiThemeDefinition[] = [
  { id: "idioteque-dark", label: "Idioteque-dark", scheme: "dark", tokens: IDIOTEQUE_DARK },
  { id: "tokyo-dark", label: "Tokyo-dark", scheme: "dark", tokens: TOKYO_DARK },
  { id: "idioteque-light", label: "Idioteque-light", scheme: "light", tokens: IDIOTEQUE_LIGHT },
  { id: "platzi", label: "Platzi", scheme: "dark", tokens: PLATZI },
  { id: "tokyo-night", label: "Tokyo Night", scheme: "dark", tokens: TOKYO_NIGHT },
  { id: "catppuccin-mocha", label: "Catppuccin Mocha", scheme: "dark", tokens: CATPPUCCIN_MOCHA },
  { id: "nord", label: "Nord", scheme: "dark", tokens: NORD },
  { id: "gruvbox-dark", label: "Gruvbox Dark", scheme: "dark", tokens: GRUVBOX_DARK },
  { id: "everforest-dark", label: "Everforest Dark", scheme: "dark", tokens: EVERFOREST_DARK },
  { id: "one-dark", label: "One Dark", scheme: "dark", tokens: ONE_DARK },
  { id: "solarized-dark", label: "Solarized Dark", scheme: "dark", tokens: SOLARIZED_DARK },
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
