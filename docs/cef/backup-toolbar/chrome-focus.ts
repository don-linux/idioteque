/** Snapshot: IDE chrome helpers for the Svelte URL bar. Not imported by the runtime. */

export type FocusOwner = "app" | "browser";

export type KeyboardTarget = {
  tagName?: string;
  isContentEditable?: boolean;
  closest?: (selector: string) => unknown;
  blur?: () => void;
  focus?: () => void;
  select?: () => void;
  selectionStart?: number | null;
  selectionEnd?: number | null;
  querySelector?: (selector: string) => KeyboardTarget | null;
};

export function isAppKeyboardTarget(el: KeyboardTarget | null | undefined): boolean {
  if (!el) return false;
  if (typeof el.closest === "function" && el.closest("[data-browser-toolbar]")) return true;
  const tag = el.tagName?.toLowerCase();
  if (tag === "input" || tag === "textarea" || tag === "select") return true;
  return el.isContentEditable === true;
}

export function shouldClaimAppFocus(owner: FocusOwner): boolean {
  return owner !== "app";
}

export function shouldHandleToolbarFocusIn(owner: FocusOwner, blocked: boolean): boolean {
  return !blocked && shouldClaimAppFocus(owner);
}

export function shouldApplyFocusUrlRequest(requested: number, lastApplied: number): boolean {
  return requested !== 0 && requested !== lastApplied;
}

export function shouldApplyForwardedKeys(owner: FocusOwner): boolean {
  return owner === "app";
}

export function shouldGiftCefFocus(owner: FocusOwner): boolean {
  return owner !== "app";
}

export function resolveChromeTarget(
  toolbar: { querySelector?: (selector: string) => KeyboardTarget | null } | null | undefined,
  next: boolean,
): KeyboardTarget | null {
  if (!toolbar?.querySelector) return null;
  return toolbar.querySelector(next ? "[data-browser-url]" : "[data-browser-chrome-last]");
}

export function shouldInvokeFocusApp(owner: FocusOwner, urlAlreadyFocused: boolean): boolean {
  return owner !== "app" || !urlAlreadyFocused;
}

export function isUrlBarElement(el: KeyboardTarget | null | undefined): boolean {
  return Boolean(el && typeof el.closest === "function" && el.closest("[data-browser-url]"));
}

export function applyTypedKeys(current: string, text: string, replace: boolean): string {
  return replace ? text : `${current}${text}`;
}

export function shouldReplaceTypedKeys(replacePending: boolean, hasSelection: boolean): boolean {
  return replacePending || hasSelection;
}

export function urlBarHasSelection(el: KeyboardTarget | null | undefined): boolean {
  const start = el?.selectionStart;
  const end = el?.selectionEnd;
  return typeof start === "number" && typeof end === "number" && start !== end;
}

/**
 * Ctrl+L: focus and select the URL field. The live `BrowserState` also kept
 * `focusUrlRequested` and called `claimUrlBar()` from the workspace layout.
 */
export function claimUrlBarSketch(options: {
  focusApp: () => void;
  url: KeyboardTarget | null;
}): void {
  options.focusApp();
  options.url?.focus?.();
  options.url?.select?.();
}
