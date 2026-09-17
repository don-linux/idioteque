const AUTHORITY_SCHEME = /^[a-z][a-z0-9+.-]*:\/\//;
const SPECIAL_SCHEME = /^(about|mailto|data|javascript|blob|file):/i;
const LOCALHOST = /^localhost(?::\d+)?(?:[/?#].*)?$/i;
const IPV4 = /^\d{1,3}(?:\.\d{1,3}){3}(?::\d+)?(?:[/?#].*)?$/;

function hostOf(value: string): string {
  return value.split(/[/?#]/, 1)[0] ?? "";
}

function hasScheme(value: string): boolean {
  return AUTHORITY_SCHEME.test(value) || SPECIAL_SCHEME.test(value);
}

/** Trimmed URL for navigation. Empty or a search phrase → `null` (no search engine). */
export function normalizeUrlInput(input: string): string | null {
  const value = input.trim();
  if (value.length === 0) return null;
  if (hasScheme(value)) return value;
  if (/\s/.test(value)) return null;
  if (LOCALHOST.test(value) || IPV4.test(value) || hostOf(value).includes(".")) {
    return `https://${value}`;
  }
  return null;
}

/**
 * Compact bar text: hide the `https://` prefix and a trailing `/`.
 * `about:blank` is an empty field.
 */
export function displayUrl(url: string): string {
  if (url === "about:blank" || url === "about:") return "";

  let shown = url;
  if (shown.startsWith("https://")) shown = shown.slice("https://".length);
  if (shown.endsWith("/")) shown = shown.slice(0, -1);
  return shown;
}
