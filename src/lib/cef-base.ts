/**
 * Extrae la versión de CEF que viaja después del `+` en el crate `cef`
 * de un `Cargo.lock` (`152.3.0+152.0.6` → `152.0.6`).
 *
 * Cargo.lock puede tener dos `[[package]]` con el mismo `name` si hay
 * versiones distintas. Si los sufijos `+` no coinciden, no hay pin.
 */
export function cefVersionsFromCargoLock(lockText: string): string[] {
  const versions: string[] = [];
  const re = /^name = "cef"\r?\nversion = "([^"]+)"/gm;
  for (const match of lockText.matchAll(re)) {
    const plus = match[1].indexOf("+");
    if (plus === -1) continue;
    const after = match[1].slice(plus + 1);
    if (after.length > 0) versions.push(after);
  }
  return versions;
}

export function cefVersionFromCargoLock(lockText: string): string | null {
  const unique = [...new Set(cefVersionsFromCargoLock(lockText.replace(/^\uFEFF/, "")))];
  return unique.length === 1 ? unique[0] : null;
}

/**
 * El `cefVersion` de base.json (`152.0.6+g708dc14+chromium-…`) debe
 * empezar por la versión del crate (`152.0.6`) seguida de `+`.
 */
export function baseMatchesCrate(baseCefVersion: string, crateCefVersion: string): boolean {
  if (!baseCefVersion || !crateCefVersion) return false;
  return baseCefVersion.startsWith(`${crateCefVersion}+`);
}

/** `152.0.6+g708dc14+chromium-152.0.7977.83` → `152.0.7977.83`. */
export function chromiumFromCefVersion(cefVersion: string): string | null {
  const marker = "+chromium-";
  const idx = cefVersion.lastIndexOf(marker);
  if (idx === -1) return null;
  const rest = cefVersion.slice(idx + marker.length);
  return rest.length > 0 ? rest : null;
}

export function pinMatchesLock(
  base: { cefVersion: string; chromiumVersion: string },
  crateCefVersion: string,
): boolean {
  if (!baseMatchesCrate(base.cefVersion, crateCefVersion)) return false;
  const chromium = chromiumFromCefVersion(base.cefVersion);
  return chromium !== null && chromium === base.chromiumVersion;
}
