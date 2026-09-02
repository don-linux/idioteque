export const FOLDER_VISIBILITY_LABEL = "Carpetas visibles";

export const FOLDER_VISIBILITY_TOAST =
  "El árbol puede saturarse. Elige las carpetas del contexto con Carpetas visibles, el icono de carpeta con + junto a idioteque.";

export interface WorkspaceView {
  path: string;
  visibleFolders: string[];
}

export function pathKeysMatch(left: string, right: string): boolean {
  if (left === right) return true;

  return trimTrailingSeps(left) === trimTrailingSeps(right);
}

export function folderName(path: string): string {
  const trimmed = trimTrailingSeps(path);
  const parts = trimmed.split(/[/\\]/).filter(Boolean);
  return parts.at(-1) ?? path;
}

export function visibilityFor(
  views: readonly WorkspaceView[],
  path: string,
): string[] | undefined {
  const match = views.find((view) => pathKeysMatch(view.path, path));
  return match?.visibleFolders;
}

export function needsFolderPicker(
  dirs: readonly string[],
  saved: string[] | undefined,
): boolean {
  return dirs.length > 0 && saved === undefined;
}

export function shouldShowFolderVisibilityToast(
  hasSubdirs: boolean,
  saved: string[] | undefined,
): boolean {
  return hasSubdirs && saved === undefined;
}

function trimTrailingSeps(path: string): string {
  const trimmed = path.replace(/[/\\]+$/, "");
  return trimmed === "" ? path : trimmed;
}
