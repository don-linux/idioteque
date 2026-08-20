export const ROUTES = {
  home: "/",
  workspace: "/workspace",
  settings: "/configuracion",
} as const;

export function settingsBackHref(hasWorkspace: boolean): string {
  return hasWorkspace ? ROUTES.workspace : ROUTES.home;
}
