export type SlotSource = "bundled" | "installed";

export interface SlotInfo {
  cefVersion: string;
  chromiumVersion: string;
  source: SlotSource;
  path: string;
  verified: boolean;
}

export interface DenyEntry {
  cefVersion: string;
  chromiumVersion: string;
  reason: string;
  at: string;
}

export interface PendingPromotion {
  cefVersion: string;
  chromiumVersion: string;
}

export interface CefRuntimeInfo {
  current: SlotInfo;
  base: SlotInfo;
  candidate: SlotInfo | null;
  denylist: DenyEntry[];
  lastCheckAt: string | null;
  pendingPromotion: PendingPromotion | null;
  hostApiVersion: number;
  platform: string;
  hostAlive: boolean;
}

export const CEF_UNAVAILABLE = "No disponible fuera de idioteque";
export const CEF_CHECKING = "Buscando…";

export function sourceLabel(source: string): "de fábrica" | "actualizado" {
  return source === "installed" ? "actualizado" : "de fábrica";
}

export function formatCheckedAt(iso: string | null): string {
  if (!iso) return "nunca";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "nunca";
  const dd = String(date.getDate()).padStart(2, "0");
  const mm = String(date.getMonth() + 1).padStart(2, "0");
  const yyyy = String(date.getFullYear());
  const hh = String(date.getHours()).padStart(2, "0");
  const min = String(date.getMinutes()).padStart(2, "0");
  return `${dd}/${mm}/${yyyy} ${hh}:${min}`;
}

export function formatDenyEntry(entry: DenyEntry): string {
  return `${entry.chromiumVersion} — ${entry.reason} — ${formatCheckedAt(entry.at)}`;
}

export function formatPendingPromotion(pending: PendingPromotion): string {
  return `Promoción pendiente: Chromium ${pending.chromiumVersion}`;
}
