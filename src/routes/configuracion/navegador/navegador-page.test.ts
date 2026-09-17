// svelte-check has no @types/node; the test runner provides node:fs at runtime.
// @ts-expect-error Node built-in used only in this guard test.
import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { formatCheckedAt, type CefRuntimeInfo } from "$lib/cef-runtime";
import {
  CEF_CHECKING,
  CEF_CYCLE_RUNNING,
  CEF_UNAVAILABLE,
  CHECK_DISABLE_MS,
  COPY,
  DENYLIST_NONE,
  RELOAD_AFTER_CHECK_MS,
  acceptRuntimeInfo,
  createNavegadorPage,
  messageFrom,
  runtimeLines,
} from "./navegador-page";

function sampleInfo(over: Partial<CefRuntimeInfo> = {}): CefRuntimeInfo {
  return {
    current: {
      cefVersion: "152.0.6+g1",
      chromiumVersion: "152.0.7977.83",
      source: "bundled",
      path: "/cef/base",
      verified: true,
    },
    base: {
      cefVersion: "152.0.6+g1",
      chromiumVersion: "152.0.7977.83",
      source: "bundled",
      path: "/cef/base",
      verified: true,
    },
    candidate: null,
    denylist: [],
    lastCheckAt: null,
    pendingPromotion: null,
    hostApiVersion: 15200,
    platform: "linux64",
    hostAlive: false,
    ...over,
  };
}

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("acceptRuntimeInfo", () => {
  it("rejects null, undefined, non-objects, and empty payloads", () => {
    expect(acceptRuntimeInfo(null)).toBeNull();
    expect(acceptRuntimeInfo(undefined)).toBeNull();
    expect(acceptRuntimeInfo("cef")).toBeNull();
    expect(acceptRuntimeInfo(152)).toBeNull();
    expect(acceptRuntimeInfo({})).toBeNull();
  });

  it("rejects a payload missing current or base slot versions", () => {
    const ok = sampleInfo();
    expect(acceptRuntimeInfo({ ...ok, current: null })).toBeNull();
    expect(acceptRuntimeInfo({ ...ok, base: undefined })).toBeNull();
    expect(
      acceptRuntimeInfo({
        ...ok,
        current: { ...ok.current, chromiumVersion: "" },
      }),
    ).toBeNull();
    expect(
      acceptRuntimeInfo({
        ...ok,
        base: { ...ok.base, cefVersion: "   " },
      }),
    ).toBeNull();
  });

  it("rejects snake_case-only slots (missing camelCase contract fields)", () => {
    expect(
      acceptRuntimeInfo({
        current: {
          cef_version: "152.0.6+g1",
          chromium_version: "152.0.7977.83",
          source: "bundled",
        },
        base: {
          cef_version: "152.0.6+g1",
          chromium_version: "152.0.7977.83",
          source: "bundled",
        },
        denylist: [],
      }),
    ).toBeNull();
  });

  it("keeps a valid payload and drops a broken pending promotion", () => {
    const accepted = acceptRuntimeInfo(
      sampleInfo({
        pendingPromotion: { cefVersion: "153.0.1", chromiumVersion: "" },
        denylist: [
          {
            cefVersion: "151.0.0",
            chromiumVersion: "151.0.1",
            reason: "health-exit-10",
            at: "2026-09-01T00:00:00.000Z",
          },
          { cefVersion: "x", chromiumVersion: "", reason: "nope", at: "" },
        ],
      }),
    );
    expect(accepted?.pendingPromotion).toBeNull();
    expect(accepted?.denylist).toHaveLength(1);
    expect(accepted?.denylist[0]?.chromiumVersion).toBe("151.0.1");
  });

  it("treats a missing denylist as none instead of rejecting the payload", () => {
    const { denylist: _drop, ...rest } = sampleInfo();
    expect(acceptRuntimeInfo(rest)?.denylist).toEqual([]);
  });
});

describe("messageFrom", () => {
  it("keeps the Spanish updater-cycle copy from a string, Error, or envelope", () => {
    expect(messageFrom(CEF_CYCLE_RUNNING)).toBe(CEF_CYCLE_RUNNING);
    expect(messageFrom(new Error(CEF_CYCLE_RUNNING))).toBe(CEF_CYCLE_RUNNING);
    expect(messageFrom({ message: CEF_CYCLE_RUNNING })).toBe(CEF_CYCLE_RUNNING);
    expect(messageFrom({ error: CEF_CYCLE_RUNNING })).toBe(CEF_CYCLE_RUNNING);
  });

  it("maps empty, non-string, and Tauri IPC noise to CEF_UNAVAILABLE", () => {
    expect(messageFrom("")).toBe(CEF_UNAVAILABLE);
    expect(messageFrom("   ")).toBe(CEF_UNAVAILABLE);
    expect(messageFrom(new Error("  "))).toBe(CEF_UNAVAILABLE);
    expect(messageFrom(null)).toBe(CEF_UNAVAILABLE);
    expect(messageFrom(undefined)).toBe(CEF_UNAVAILABLE);
    expect(messageFrom(12)).toBe(CEF_UNAVAILABLE);
    expect(messageFrom({ message: "  " })).toBe(CEF_UNAVAILABLE);
    expect(messageFrom("Command cef_check_updates not found")).toBe(
      CEF_UNAVAILABLE,
    );
    expect(messageFrom("ipc invoke failed")).toBe(CEF_UNAVAILABLE);
    expect(messageFrom("The IPC function `invoke` is not available")).toBe(
      CEF_UNAVAILABLE,
    );
  });

  it("does not leak English IPC wording", () => {
    const copy = messageFrom("Command cef_runtime_info not found");
    expect(copy).toBe(CEF_UNAVAILABLE);
    expect(copy).not.toMatch(/command|not found|invoke|ipc/i);
  });
});

describe("runtimeLines / COPY", () => {
  it("uses the contract Spanish copy for factory, never, and empty denylist", () => {
    const lines = runtimeLines(sampleInfo());
    expect(COPY.heading).toBe("Navegador");
    expect(COPY.lead).toBe(
      "Motor Chromium embebido. No hay nada que guardar aquí: es información y un botón.",
    );
    expect(COPY.loading).toBe("Cargando…");
    expect(COPY.check).toBe("Buscar actualización");
    expect(DENYLIST_NONE).toBe("ninguna");
    expect(CEF_CHECKING).toBe("Buscando…");
    expect(CEF_UNAVAILABLE).toBe("No disponible fuera de idioteque");
    expect(lines.chromium).toBe("Chromium actual: 152.0.7977.83 (de fábrica)");
    expect(lines.cef).toBe("CEF: 152.0.6+g1");
    expect(lines.base).toBe("Base de fábrica: Chromium 152.0.7977.83");
    expect(lines.lastCheck).toBe("Última comprobación: nunca");
    expect(lines.pending).toBeNull();
    expect(lines.denylistLabel).toBe("Versiones descartadas:");
    expect(lines.denylist).toEqual([]);
  });

  it("labels an installed slot and formats pending + denylist rows", () => {
    const at = "2026-09-16T11:05:00.000Z";
    const lines = runtimeLines(
      sampleInfo({
        current: {
          cefVersion: "153.0.1+gabc",
          chromiumVersion: "153.0.8000.10",
          source: "installed",
          path: "/cef/current",
          verified: true,
        },
        lastCheckAt: at,
        pendingPromotion: {
          cefVersion: "153.0.1+gabc",
          chromiumVersion: "153.0.8000.10",
        },
        denylist: [
          {
            cefVersion: "151.0.0",
            chromiumVersion: "151.0.1",
            reason: "health-exit-10",
            at,
          },
        ],
      }),
    );
    expect(lines.chromium).toBe("Chromium actual: 153.0.8000.10 (actualizado)");
    expect(lines.pending).toBe("Promoción pendiente: Chromium 153.0.8000.10");
    expect(lines.lastCheck).toBe(`Última comprobación: ${formatCheckedAt(at)}`);
    expect(lines.denylist).toEqual([
      `151.0.1 — health-exit-10 — ${formatCheckedAt(at)}`,
    ]);
  });
});

describe("createNavegadorPage — missing runtime info", () => {
  it("shows CEF_UNAVAILABLE when cef_runtime_info rejects, without leaking English", async () => {
    const invoke = vi.fn().mockRejectedValue("Command cef_runtime_info not found");
    const page = createNavegadorPage({ invoke });
    await page.load();
    const snap = page.snapshot();
    expect(snap.info).toBeNull();
    expect(snap.error).toBe(CEF_UNAVAILABLE);
    expect(snap.error).not.toMatch(/command|not found/i);
  });

  it("treats a successful invoke with no usable payload as unavailable, not loading", async () => {
    for (const payload of [null, undefined, {}, { current: { cefVersion: "1" } }]) {
      const invoke = vi.fn().mockResolvedValue(payload);
      const page = createNavegadorPage({ invoke });
      await page.load();
      expect(page.snapshot(), String(payload)).toEqual({
        info: null,
        error: CEF_UNAVAILABLE,
        status: null,
        checking: false,
      });
    }
  });

  it("accepts a full contract payload", async () => {
    const payload = sampleInfo({ lastCheckAt: "2026-09-16T11:05:00.000Z" });
    const invoke = vi.fn().mockResolvedValue(payload);
    const page = createNavegadorPage({ invoke });
    await page.load();
    expect(page.snapshot().error).toBeNull();
    expect(page.snapshot().info?.current.chromiumVersion).toBe("152.0.7977.83");
    expect(page.snapshot().info?.lastCheckAt).toBe(payload.lastCheckAt);
  });

  it("ignores a late resolve and a late reject after dispose", async () => {
    const pending = Promise.withResolvers<CefRuntimeInfo>();
    const invoke = vi.fn().mockReturnValue(pending.promise);
    const page = createNavegadorPage({ invoke });
    const loading = page.load();
    page.dispose();
    pending.resolve(sampleInfo());
    await loading;
    expect(page.snapshot().info).toBeNull();
    expect(page.snapshot().error).toBeNull();

    const rejected = Promise.withResolvers<never>();
    const invoke2 = vi.fn().mockReturnValue(rejected.promise);
    const page2 = createNavegadorPage({ invoke: invoke2 });
    const loading2 = page2.load();
    page2.dispose();
    rejected.reject("boom");
    await loading2;
    expect(page2.snapshot().error).toBeNull();
  });
});

describe("createNavegadorPage — update check errors", () => {
  it("sets Buscando…, holds the button for 3s, then reloads and clears the hint", async () => {
    vi.useFakeTimers();
    const payload = sampleInfo({ lastCheckAt: "2026-09-17T10:00:00.000Z" });
    const invoke = vi.fn(async (command: string) => {
      if (command === "cef_check_updates") return undefined;
      return payload;
    });
    const page = createNavegadorPage({ invoke });
    await page.checkUpdates();
    expect(page.snapshot().checking).toBe(true);
    expect(page.snapshot().status).toBe(CEF_CHECKING);
    expect(invoke).toHaveBeenCalledWith("cef_check_updates");

    await vi.advanceTimersByTimeAsync(CHECK_DISABLE_MS);
    expect(page.snapshot().checking).toBe(false);
    expect(page.snapshot().status).toBe(CEF_CHECKING);
    expect(invoke).not.toHaveBeenCalledWith("cef_runtime_info");

    await vi.advanceTimersByTimeAsync(RELOAD_AFTER_CHECK_MS - CHECK_DISABLE_MS);
    expect(page.snapshot().status).toBeNull();
    expect(page.snapshot().info?.lastCheckAt).toBe(payload.lastCheckAt);
  });

  it("keeps the Spanish cycle error after reload and does not treat it as Buscando…", async () => {
    vi.useFakeTimers();
    const invoke = vi.fn(async (command: string) => {
      if (command === "cef_check_updates") throw CEF_CYCLE_RUNNING;
      return sampleInfo();
    });
    const page = createNavegadorPage({ invoke });
    await page.checkUpdates();
    expect(page.snapshot().status).toBe(CEF_CYCLE_RUNNING);
    await vi.advanceTimersByTimeAsync(RELOAD_AFTER_CHECK_MS);
    expect(page.snapshot().status).toBe(CEF_CYCLE_RUNNING);
    expect(page.snapshot().info?.current.chromiumVersion).toBe("152.0.7977.83");
  });

  it("maps a missing-command check failure to CEF_UNAVAILABLE", async () => {
    const invoke = vi.fn().mockRejectedValue("Command cef_check_updates not found");
    const page = createNavegadorPage({ invoke });
    await page.checkUpdates();
    expect(page.snapshot().status).toBe(CEF_UNAVAILABLE);
    expect(page.snapshot().checking).toBe(true);
  });

  it("does not start a second cycle while the button is held", async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    const page = createNavegadorPage({ invoke });
    await page.checkUpdates();
    await page.checkUpdates();
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("does not invoke after dispose, and drops an in-flight check", async () => {
    vi.useFakeTimers();
    const pending = Promise.withResolvers<undefined>();
    const invoke = vi.fn().mockReturnValue(pending.promise);
    const page = createNavegadorPage({ invoke });
    const running = page.checkUpdates();
    page.dispose();
    pending.resolve(undefined);
    await running;
    await vi.runAllTimersAsync();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("cef_check_updates");
    expect(page.snapshot().status).toBe(CEF_CHECKING);

    const idle = createNavegadorPage({ invoke: vi.fn() });
    idle.dispose();
    await idle.checkUpdates();
    expect(idle.snapshot().checking).toBe(false);
  });

  it("cancels the first reload timer if the user checks again after the 3s hold", async () => {
    vi.useFakeTimers();
    const commands: string[] = [];
    const invoke = vi.fn(async (command: string) => {
      commands.push(command);
      if (command === "cef_runtime_info") return sampleInfo();
      return undefined;
    });
    const page = createNavegadorPage({ invoke });
    await page.checkUpdates();
    await vi.advanceTimersByTimeAsync(CHECK_DISABLE_MS);
    await page.checkUpdates();
    await vi.advanceTimersByTimeAsync(RELOAD_AFTER_CHECK_MS - 1);
    expect(commands.filter((command) => command === "cef_runtime_info")).toEqual(
      [],
    );
    await vi.advanceTimersByTimeAsync(1);
    expect(commands.filter((command) => command === "cef_runtime_info")).toEqual(
      ["cef_runtime_info"],
    );
  });
});

describe("+page.svelte wiring", () => {
  const source = readFileSync(new URL("./+page.svelte", import.meta.url), "utf8");

  it("renders through the extracted session and copy helpers", () => {
    expect(source).toContain("createNavegadorPage");
    expect(source).toContain("runtimeLines");
    expect(source).toContain("COPY");
    expect(source).toContain("DENYLIST_NONE");
    expect(source).not.toMatch(/invoke<CefRuntimeInfo>/);
    expect(source).not.toContain("messageFrom");
    expect(source).not.toMatch(/error = caught/);
  });
});
