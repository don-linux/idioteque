import { describe, expect, it } from "vitest";
import {
  ISSUES_URL,
  incompatibleDetail,
  incompatibleMessage,
  issueUrl,
  updatedMessage,
  type CefIncompatibleEvent,
  type CefUpdatedEvent,
} from "./cef-notices";

const updated: CefUpdatedEvent = {
  kind: "updated",
  chromium: "153.0.8000.10",
  cef: "153.0.1+gabc",
};

const incompatible: CefIncompatibleEvent = {
  kind: "incompatible",
  candidateChromium: "153.0.8000.10",
  candidateCef: "153.0.1+gabc",
  currentChromium: "152.0.7977.83",
  currentCef: "152.0.6+gdef",
  reason: "health-exit-10",
};

describe("updatedMessage", () => {
  it("names the new Chromium version", () => {
    expect(updatedMessage(updated)).toBe("Se ha actualizado a Chromium 153.0.8000.10");
  });
});

describe("incompatibleMessage", () => {
  it("uses the exact copy with accents and a blank line", () => {
    expect(incompatibleMessage(incompatible)).toBe(
      "Se intentó actualizar a Chromium 153.0.8000.10, no es compatible con esta versión de idioteque.\n\nChromium continuará en 152.0.7977.83. Puedes abrir un issue reportando:",
    );
  });

  it("never says the candidate failed to compile", () => {
    expect(incompatibleMessage(incompatible).toLowerCase()).not.toContain("compil");
  });
});

describe("incompatibleDetail", () => {
  it("lists candidate and current Chromium versions", () => {
    expect(incompatibleDetail(incompatible)).toBe(
      "Candidato: 153.0.8000.10\nActual: 152.0.7977.83",
    );
  });

  it("never says the candidate failed to compile", () => {
    expect(incompatibleDetail(incompatible).toLowerCase()).not.toContain("compil");
  });
});

describe("issueUrl", () => {
  const ctx = {
    idiotequeVersion: "0.1.0",
    hostApiVersion: 15200,
    platform: "linux64",
  };

  it("opens a new GitHub issue with encoded title and body", () => {
    const url = new URL(issueUrl(incompatible, ctx));

    expect(`${url.origin}${url.pathname}`).toBe(ISSUES_URL);
    expect(url.searchParams.get("title")).toBe(
      "CEF 153.0.8000.10 no compatible con idioteque 0.1.0",
    );
    expect(url.searchParams.get("body")).toBe(
      [
        "Candidato CEF: 153.0.1+gabc",
        "Candidato Chromium: 153.0.8000.10",
        "Actual CEF: 152.0.6+gdef",
        "Actual Chromium: 152.0.7977.83",
        "hostApiVersion: 15200",
        "reason: health-exit-10",
        "plataforma: linux64",
      ].join("\n"),
    );
  });
});
