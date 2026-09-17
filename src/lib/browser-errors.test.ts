import { describe, expect, it } from "vitest";
import { RENDER_CRASHED, clearsRenderCrash, renderCrashedMessage } from "./browser-errors";

describe("renderCrashedMessage", () => {
  it("adjunta el status del host entre paréntesis", () => {
    expect(renderCrashedMessage("oom")).toBe(`${RENDER_CRASHED} (oom)`);
    expect(renderCrashedMessage("  crashed ")).toBe(`${RENDER_CRASHED} (crashed)`);
  });

  it("sin status deja el mensaje base", () => {
    expect(renderCrashedMessage("")).toBe(RENDER_CRASHED);
    expect(renderCrashedMessage("   ")).toBe(RENDER_CRASHED);
  });
});

describe("clearsRenderCrash", () => {
  it("solo limpia el error de renderer crasheado", () => {
    expect(clearsRenderCrash(renderCrashedMessage("oom"))).toBe(true);
    expect(clearsRenderCrash(RENDER_CRASHED)).toBe(true);
  });

  it("respeta load-error, fatal y exit", () => {
    expect(clearsRenderCrash(null)).toBe(false);
    expect(clearsRenderCrash("ERR_INSUFFICIENT_RESOURCES")).toBe(false);
    expect(clearsRenderCrash("El sandbox de Chromium no está disponible")).toBe(false);
    expect(clearsRenderCrash("El navegador se cerró inesperadamente")).toBe(false);
  });
});
