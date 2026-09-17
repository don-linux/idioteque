/**
 * Extrae la versión de CEF que viaja después del `+` en el crate `cef`
 * de un `Cargo.lock` (`152.3.0+152.0.6` → `152.0.6`).
 */
export function cefVersionFromCargoLock(lockText: string): string | null {
  const match = lockText.match(/^name = "cef"\r?\nversion = "([^"]+)"/m);
  if (!match) return null;
  const plus = match[1].indexOf("+");
  if (plus === -1) return null;
  const after = match[1].slice(plus + 1);
  return after.length > 0 ? after : null;
}

/**
 * El `cefVersion` de base.json (`152.0.6+g708dc14+chromium-…`) debe
 * empezar por la versión del crate (`152.0.6`) seguida de `+`.
 */
export function baseMatchesCrate(baseCefVersion: string, crateCefVersion: string): boolean {
  if (!baseCefVersion || !crateCefVersion) return false;
  return baseCefVersion.startsWith(`${crateCefVersion}+`);
}
