import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { BrowserBoot, BrowserBounds, BrowserEvent } from "./browser.svelte";
import { surface } from "./workspace-surface.svelte";

/**
 * Vitest has no svelte plugin. `.svelte.ts` runes stay as function calls,
 * so identity stubs are enough to exercise the state machine.
 */
const tauri = vi.hoisted(() => {
  const globals = globalThis as typeof globalThis & {
    $state: <T>(value: T) => T;
    $derived: <T>(value: T) => T;
  };
  globals.$state = (value) => value;
  globals.$derived = (value) => value;

  class Channel<T> {
    onmessage: (event: T) => void = () => {};
  }

  return { Channel, invoke: vi.fn() };
});

vi.mock("@tauri-apps/api/core", () => ({
  Channel: tauri.Channel,
  invoke: tauri.invoke,
}));

const { browser } = await import("./browser.svelte");

const BOUNDS: BrowserBounds = { x: 8, y: 40, w: 1200, h: 700 };
const INCOMPATIBLE = "El motor Chromium no es compatible con esta versión de idioteque";
const SANDBOX = "El sandbox de Chromium no está disponible";
const NEED_X11 = "El navegador necesita X11";
const UNEXPECTED = "El navegador se cerró inesperadamente";

function boot(overrides: Partial<BrowserBoot> = {}): BrowserBoot {
  return {
    cef: "152.0.6+g708dc14+chromium-152.0.7977.83",
    chromium: "152.0.7977.83",
    apiVersion: 15200,
    source: "bundled",
    noSandbox: false,
    ...overrides,
  };
}

function ready(overrides: Partial<Extract<BrowserEvent, { event: "ready" }>> = {}): BrowserEvent {
  return {
    event: "ready",
    cef: "152.0.6+g708dc14+chromium-152.0.7977.83",
    chromium: "152.0.7977.83",
    apiVersion: 15200,
    xid: 0x2a,
    ...overrides,
  };
}

type ChannelLike = { onmessage: (event: BrowserEvent) => void };

type SpawnGate = {
  channel: ChannelLike | null;
  resolve: ((value: BrowserBoot) => void) | null;
  reject: ((error: unknown) => void) | null;
};

function installInvoke(mode: "resolve" | "hang" = "resolve"): SpawnGate {
  const gate: SpawnGate = { channel: null, resolve: null, reject: null };

  tauri.invoke.mockImplementation((cmd: string, args?: { onEvent?: ChannelLike }) => {
    if (cmd === "browser_spawn") {
      gate.channel = args?.onEvent ?? null;
      if (mode === "hang") {
        return new Promise<BrowserBoot>((resolve, reject) => {
          gate.resolve = resolve;
          gate.reject = reject;
        });
      }
      return Promise.resolve(boot());
    }
    return Promise.resolve();
  });

  return gate;
}

async function spawnLive(gate = installInvoke("resolve")): Promise<SpawnGate> {
  const pending = browser.spawn(BOUNDS, 1);
  expect(gate.channel).toBeTruthy();
  gate.channel?.onmessage(ready());
  await pending;
  expect(browser.alive).toBe(true);
  expect(browser.booting).toBe(false);
  return gate;
}

describe("browser.svelte.ts state", () => {
  beforeEach(() => {
    tauri.invoke.mockReset();
    surface.set("editor");
  });

  afterEach(async () => {
    installInvoke("resolve");
    await browser.teardown();
    surface.set("editor");
  });

  describe("#gen stale events", () => {
    it("drops nav/title/ready from a previous spawn generation", async () => {
      const first = await spawnLive();
      first.channel?.onmessage({
        event: "nav",
        url: "https://one.example/",
        canGoBack: true,
        canGoForward: false,
        loading: false,
      });
      first.channel?.onmessage({ event: "title", title: "one" });
      expect(browser.url).toBe("https://one.example/");
      expect(browser.title).toBe("one");

      await browser.teardown();
      expect(browser.alive).toBe(false);
      expect(browser.url).toBe("about:blank");
      expect(browser.title).toBe("");

      const second = await spawnLive();
      first.channel?.onmessage({
        event: "nav",
        url: "https://stale.example/",
        canGoBack: true,
        canGoForward: true,
        loading: true,
      });
      first.channel?.onmessage({ event: "title", title: "stale" });
      first.channel?.onmessage(ready({ xid: 99 }));
      first.channel?.onmessage({
        event: "load-error",
        code: -105,
        text: "ERR_NAME_NOT_RESOLVED",
        url: "https://stale.example/",
      });
      first.channel?.onmessage({ event: "fatal", message: "stale fatal", code: 11 });
      first.channel?.onmessage({ event: "exit", code: 15 });

      expect(browser.alive).toBe(true);
      expect(browser.url).toBe("about:blank");
      expect(browser.title).toBe("");
      expect(browser.error).toBeNull();
      expect(browser.canGoBack).toBe(false);
      expect(browser.canGoForward).toBe(false);
      expect(browser.loading).toBe(false);

      second.channel?.onmessage({
        event: "nav",
        url: "https://two.example/",
        canGoBack: false,
        canGoForward: false,
        loading: false,
      });
      expect(browser.url).toBe("https://two.example/");
    });

    it("ignores a late browser_spawn resolve from a superseded generation", async () => {
      const hung = installInvoke("hang");
      const first = browser.spawn(BOUNDS, 1);
      expect(browser.booting).toBe(true);

      await browser.teardown();
      expect(browser.booting).toBe(false);
      expect(browser.boot).toBeNull();

      hung.resolve?.(boot({ noSandbox: true, source: "installed" }));
      await first;

      expect(browser.boot).toBeNull();
      expect(browser.noSandbox).toBe(false);
      expect(browser.alive).toBe(false);
      expect(browser.error).toBeNull();
    });

    it("ignores a late browser_spawn reject from a superseded generation", async () => {
      const hung = installInvoke("hang");
      const first = browser.spawn(BOUNDS, 1);
      await browser.teardown();

      hung.reject?.(new Error("spawn aborted"));
      await first;

      expect(browser.error).toBeNull();
      expect(browser.booting).toBe(false);
    });
  });

  describe("load-error -3", () => {
    it("ignores ERR_ABORTED (-3) and keeps loading plus the previous error", async () => {
      const gate = await spawnLive();
      gate.channel?.onmessage({
        event: "nav",
        url: "https://aborted.example/",
        canGoBack: false,
        canGoForward: false,
        loading: true,
      });
      gate.channel?.onmessage({
        event: "load-error",
        code: -105,
        text: "ERR_NAME_NOT_RESOLVED",
        url: "https://aborted.example/",
      });
      expect(browser.error).toBe("ERR_NAME_NOT_RESOLVED");
      expect(browser.loading).toBe(false);

      gate.channel?.onmessage({
        event: "nav",
        url: "https://next.example/",
        canGoBack: false,
        canGoForward: false,
        loading: true,
      });
      expect(browser.loading).toBe(true);

      gate.channel?.onmessage({
        event: "load-error",
        code: -3,
        text: "ERR_ABORTED",
        url: "https://next.example/",
      });
      gate.channel?.onmessage({
        event: "load-error",
        code: -3,
        text: "",
        url: "https://next.example/",
      });

      expect(browser.loading).toBe(true);
      expect(browser.error).toBe("ERR_NAME_NOT_RESOLVED");
      expect(browser.alive).toBe(true);
    });

    it("treats every other load-error code as a page failure", async () => {
      const gate = await spawnLive();
      gate.channel?.onmessage({
        event: "load-error",
        code: 3,
        text: "not-aborted-positive-3",
        url: "https://x.example/",
      });
      expect(browser.error).toBe("not-aborted-positive-3");
      expect(browser.loading).toBe(false);

      gate.channel?.onmessage({
        event: "load-error",
        code: -2,
        text: "",
        url: "https://x.example/",
      });
      expect(browser.error).toBe("No se pudo cargar la página");

      gate.channel?.onmessage({
        event: "load-error",
        code: 0,
        text: "ERR_FAILED",
        url: "https://x.example/",
      });
      expect(browser.error).toBe("ERR_FAILED");
    });
  });

  describe("exit → copy", () => {
    it("maps host exit codes to the Spanish contract copy", async () => {
      const cases: Array<[number, string | null]> = [
        [0, null],
        [10, INCOMPATIBLE],
        [13, INCOMPATIBLE],
        [14, INCOMPATIBLE],
        [15, SANDBOX],
        [16, NEED_X11],
        [1, UNEXPECTED],
        [2, UNEXPECTED],
        [11, UNEXPECTED],
        [12, UNEXPECTED],
        [99, UNEXPECTED],
      ];

      for (const [code, copy] of cases) {
        const gate = await spawnLive();
        gate.channel?.onmessage({ event: "exit", code });
        expect(browser.alive, `exit ${code} alive`).toBe(false);
        expect(browser.booting, `exit ${code} booting`).toBe(false);
        expect(browser.loading, `exit ${code} loading`).toBe(false);
        expect(browser.error, `exit ${code} copy`).toBe(copy);
        await browser.teardown();
      }
    });

    it("does not keep a more specific fatal when a known exit copy applies", async () => {
      const gate = await spawnLive();
      gate.channel?.onmessage({ event: "fatal", message: "chrome-sandbox: SUID", code: 15 });
      gate.channel?.onmessage({ event: "exit", code: 15 });
      expect(browser.error).toBe(SANDBOX);
    });
  });

  describe("hidden fatal on sandbox retry", () => {
    it("keeps Arrancando (booting, no error) if ADE leaks fatal 15/11/1 before ready", async () => {
      const hung = installInvoke("hang");
      const pending = browser.spawn(BOUNDS, 1);
      expect(browser.booting).toBe(true);
      expect(hung.channel).toBeTruthy();

      hung.channel?.onmessage({ event: "fatal", message: "No usable sandbox!", code: 15 });
      expect(browser.booting).toBe(true);
      expect(browser.alive).toBe(false);
      expect(browser.error).toBeNull();

      hung.channel?.onmessage({ event: "fatal", message: "initialize failed", code: 11 });
      hung.channel?.onmessage({ event: "fatal", message: "aborted", code: 1 });
      expect(browser.booting).toBe(true);
      expect(browser.error).toBeNull();

      // A leaked retry fatal must not look like a finished boot (that would
      // let BrowserView call spawn() again while ADE is retrying).
      const again = browser.spawn(BOUNDS, 2);
      await again;
      expect(tauri.invoke.mock.calls.filter((call) => call[0] === "browser_spawn")).toHaveLength(1);

      hung.resolve?.(boot({ noSandbox: true }));
      hung.channel?.onmessage(ready());
      await pending;

      expect(browser.alive).toBe(true);
      expect(browser.booting).toBe(false);
      expect(browser.error).toBeNull();
      expect(browser.noSandbox).toBe(true);
    });

    it("still shows a non-retry fatal (10/13/14/16) while booting", async () => {
      const hung = installInvoke("hang");
      const pending = browser.spawn(BOUNDS, 1);

      hung.channel?.onmessage({ event: "fatal", message: "api hash rejected", code: 10 });
      expect(browser.booting).toBe(false);
      expect(browser.alive).toBe(false);
      expect(browser.error).toBe("api hash rejected");

      hung.resolve?.(boot());
      await pending;
    });

    it("after a hidden retry fatal, exit 15 uses the sandbox copy (retry failed)", async () => {
      const hung = installInvoke("hang");
      const pending = browser.spawn(BOUNDS, 1);

      hung.channel?.onmessage({ event: "fatal", message: "No usable sandbox!", code: 15 });
      expect(browser.error).toBeNull();
      expect(browser.booting).toBe(true);

      hung.channel?.onmessage({ event: "exit", code: 15 });
      hung.reject?.(new Error("host exited 15"));
      await pending;

      expect(browser.booting).toBe(false);
      expect(browser.alive).toBe(false);
      expect(browser.error).toBe(SANDBOX);
    });

    it("ignores a late first-attempt fatal after ready from the same generation", async () => {
      const hung = installInvoke("hang");
      const pending = browser.spawn(BOUNDS, 1);
      hung.channel?.onmessage({ event: "fatal", message: "No usable sandbox!", code: 15 });
      hung.resolve?.(boot({ noSandbox: true }));
      hung.channel?.onmessage(ready());
      await pending;

      hung.channel?.onmessage({ event: "fatal", message: "No usable sandbox!", code: 15 });
      hung.channel?.onmessage({ event: "fatal", message: "initialize failed", code: 11 });

      expect(browser.alive).toBe(true);
      expect(browser.error).toBeNull();
    });

    it("hides a flushed retry fatal that arrives only after ready (ADE swallowed the first)", async () => {
      const gate = await spawnLive();
      gate.channel?.onmessage({ event: "fatal", message: "No usable sandbox!", code: 15 });
      gate.channel?.onmessage({ event: "fatal", message: "initialize failed", code: 11 });
      expect(browser.alive).toBe(true);
      expect(browser.error).toBeNull();
    });

    it("does not let a late retry fatal overwrite the exit copy", async () => {
      const hung = installInvoke("hang");
      const pending = browser.spawn(BOUNDS, 1);
      hung.channel?.onmessage({ event: "fatal", message: "No usable sandbox!", code: 15 });
      hung.channel?.onmessage({ event: "exit", code: 15 });
      hung.reject?.(new Error("host exited 15"));
      await pending;

      hung.channel?.onmessage({ event: "fatal", message: "No usable sandbox!", code: 15 });
      expect(browser.error).toBe(SANDBOX);
    });
  });

  describe("teardown vs Channel", () => {
    it("bumps #gen before kill so in-flight Channel events cannot resurrect state", async () => {
      const hung = installInvoke("hang");
      const pending = browser.spawn(BOUNDS, 1);
      hung.channel?.onmessage(ready());
      hung.resolve?.(boot());
      await pending;

      hung.channel?.onmessage({
        event: "nav",
        url: "https://live.example/",
        canGoBack: true,
        canGoForward: false,
        loading: true,
      });
      hung.channel?.onmessage({ event: "title", title: "live" });

      const teardown = browser.teardown();
      hung.channel?.onmessage({
        event: "nav",
        url: "https://after-teardown.example/",
        canGoBack: true,
        canGoForward: true,
        loading: true,
      });
      hung.channel?.onmessage({ event: "title", title: "ghost" });
      hung.channel?.onmessage({ event: "render-crashed", status: "oom" });
      hung.channel?.onmessage({ event: "fatal", message: "late fatal", code: 11 });
      hung.channel?.onmessage({ event: "exit", code: 15 });
      hung.channel?.onmessage(ready({ xid: 7 }));
      await teardown;

      expect(browser.started).toBe(false);
      expect(browser.alive).toBe(false);
      expect(browser.booting).toBe(false);
      expect(browser.pendingSpawn).toBe(false);
      expect(browser.error).toBeNull();
      expect(browser.url).toBe("about:blank");
      expect(browser.inputUrl).toBe("");
      expect(browser.title).toBe("");
      expect(browser.loading).toBe(false);
      expect(browser.canGoBack).toBe(false);
      expect(browser.canGoForward).toBe(false);
      expect(browser.boot).toBeNull();
      expect(browser.noSandbox).toBe(false);
      expect(tauri.invoke).toHaveBeenCalledWith("browser_kill");
    });

    it("leaves the editor surface and still kills when Channel is already silent", async () => {
      browser.enter();
      expect(surface.current).toBe("browser");
      const gate = await spawnLive();
      gate.channel = null;
      await browser.teardown();
      expect(surface.current).toBe("editor");
      expect(tauri.invoke).toHaveBeenCalledWith("browser_kill");
    });

    it("respawn starts a new generation; the old Channel cannot copy into it", async () => {
      const first = await spawnLive();
      first.channel?.onmessage({
        event: "nav",
        url: "https://old.example/",
        canGoBack: true,
        canGoForward: false,
        loading: false,
      });

      installInvoke("resolve");
      await browser.respawn();
      expect(browser.pendingSpawn).toBe(true);
      expect(browser.started).toBe(true);
      expect(browser.alive).toBe(false);
      expect(surface.current).toBe("browser");

      const second = await spawnLive();
      first.channel?.onmessage({ event: "exit", code: 16 });
      first.channel?.onmessage({
        event: "load-error",
        code: -105,
        text: "stale",
        url: "https://old.example/",
      });

      expect(browser.alive).toBe(true);
      expect(browser.error).toBeNull();
      expect(browser.url).toBe("about:blank");

      second.channel?.onmessage({
        event: "nav",
        url: "https://new.example/",
        canGoBack: false,
        canGoForward: false,
        loading: false,
      });
      expect(browser.url).toBe("https://new.example/");
    });
  });
});
