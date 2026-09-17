export type CefUpdateEvent =
  | { kind: "updated"; chromium: string; cef: string }
  | {
      kind: "incompatible";
      candidateChromium: string;
      candidateCef: string;
      currentChromium: string;
      currentCef: string;
      reason: string;
    };

export type CefIncompatibleEvent = Extract<CefUpdateEvent, { kind: "incompatible" }>;
export type CefUpdatedEvent = Extract<CefUpdateEvent, { kind: "updated" }>;

export const ISSUES_URL = "https://github.com/don-linux/idioteque/issues/new";

export interface IssueContext {
  idiotequeVersion: string;
  hostApiVersion: number;
  platform: string;
}

export function updatedMessage(e: CefUpdatedEvent): string {
  return `Se ha actualizado a Chromium ${e.chromium}`;
}

export function incompatibleMessage(e: CefIncompatibleEvent): string {
  return `Se intentó actualizar a Chromium ${e.candidateChromium}, no es compatible con esta versión de idioteque.\n\nChromium continuará en ${e.currentChromium}. Puedes abrir un issue reportando:`;
}

export function incompatibleDetail(e: CefIncompatibleEvent): string {
  return `Candidato: ${e.candidateChromium}\nActual: ${e.currentChromium}`;
}

export function issueUrl(e: CefIncompatibleEvent, ctx: IssueContext): string {
  const title = `CEF ${e.candidateChromium} no compatible con idioteque ${ctx.idiotequeVersion}`;
  const body = [
    `Candidato CEF: ${e.candidateCef}`,
    `Candidato Chromium: ${e.candidateChromium}`,
    `Actual CEF: ${e.currentCef}`,
    `Actual Chromium: ${e.currentChromium}`,
    `hostApiVersion: ${ctx.hostApiVersion}`,
    `reason: ${e.reason}`,
    `plataforma: ${ctx.platform}`,
  ].join("\n");
  return `${ISSUES_URL}?title=${encodeURIComponent(title)}&body=${encodeURIComponent(body)}`;
}
