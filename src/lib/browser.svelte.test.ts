import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { BrowserBoot, BrowserEvent } from "./browser.svelte";

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

const toasts = vi.hoisted(() => ({
  notice: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  Channel: tauri.Channel,
  invoke: tauri.invoke,
}));

vi.mock("$lib/toast.svelte", () => ({ toasts }));

const { browser } = await import("./browser.svelte");

const INCOMPATIBLE = "El motor Chromium no es compatible con esta versión de idioteque";
const SANDBOX = "El sandbox de Chromium no está disponible";
const NO_DISPLAY = "sin compositor Wayland";
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
  const pending = browser.spawn();
  expect(gate.channel).toBeTruthy();
  gate.channel?.onmessage(ready());
  await pending;
  expect(browser.alive).toBe(true);
  expect(browser.booting).toBe(false);
  expect(browser.visible).toBe(true);
  return gate;
}

describe("browser window machine", () => {
  beforeEach(() => {
    tauri.invoke.mockReset();
    toasts.notice.mockReset();
  });

  afterEach(async () => {
    installInvoke("resolve");
    await browser.teardown();
  });

  it("spawns without hole bounds and marks the window visible on ready", async () => {
    const gate = await spawnLive();
    const spawn = tauri.invoke.mock.calls.find((call) => call[0] === "browser_spawn");
    expect(spawn?.[1]).toEqual({
      url: "about:blank",
      onEvent: gate.channel,
    });
    expect(browser.started).toBe(true);
    expect(browser.boot?.apiVersion).toBe(15200);
  });

  it("toggles hide without teardown and show without a second spawn", async () => {
    await spawnLive();
    tauri.invoke.mockClear();
    installInvoke("resolve");

    await browser.toggle();
    expect(browser.alive).toBe(true);
    expect(browser.visible).toBe(false);
    expect(tauri.invoke).toHaveBeenCalledWith("browser_set_visible", { visible: false });
    expect(tauri.invoke.mock.calls.map((call) => call[0])).not.toContain("browser_kill");
    expect(tauri.invoke.mock.calls.map((call) => call[0])).not.toContain("browser_spawn");

    await browser.toggle();
    expect(browser.visible).toBe(true);
    expect(tauri.invoke).toHaveBeenCalledWith("browser_set_visible", { visible: true });
  });

  it("teardown kills the host (Inicio)", async () => {
    await spawnLive();
    await browser.teardown();
    expect(browser.alive).toBe(false);
    expect(browser.visible).toBe(false);
    expect(browser.started).toBe(false);
    expect(browser.url).toBe("about:blank");
    expect(browser.title).toBe("");
    expect(tauri.invoke).toHaveBeenCalledWith("browser_kill");
  });

  it("enter spawns when the window is down", async () => {
    const gate = installInvoke("resolve");
    const pending = browser.enter();
    expect(browser.booting).toBe(true);
    gate.channel?.onmessage(ready());
    await pending;
    expect(browser.alive).toBe(true);
    expect(browser.visible).toBe(true);
  });

  it("drops nav/title/ready from a previous spawn generation", async () => {
    const first = installInvoke("hang");
    const pending = browser.spawn();
    await browser.teardown();
    first.channel?.onmessage({
      event: "nav",
      url: "https://one.example/",
      canGoBack: true,
      canGoForward: false,
      loading: false,
    });
    first.channel?.onmessage({ event: "title", title: "one" });
    first.channel?.onmessage(ready());
    first.resolve?.(boot());
    await pending;
    expect(browser.alive).toBe(false);
    expect(browser.url).toBe("about:blank");
    expect(browser.title).toBe("");
  });

  it("ignores a late browser_spawn resolve from a superseded generation", async () => {
    const first = installInvoke("hang");
    const pending = browser.spawn();
    expect(browser.booting).toBe(true);
    await browser.teardown();
    first.resolve?.(boot());
    await pending;
    expect(browser.booting).toBe(false);
    expect(browser.boot).toBeNull();
    expect(browser.alive).toBe(false);
  });

  it("ignores ERR_ABORTED (-3) and keeps loading plus the previous error", async () => {
    const gate = await spawnLive();
    gate.channel?.onmessage({
      event: "nav",
      url: "https://two.example/",
      canGoBack: false,
      canGoForward: false,
      loading: true,
    });
    gate.channel?.onmessage({
      event: "load-error",
      code: -105,
      text: "ERR_NAME_NOT_RESOLVED",
      url: "https://two.example/",
    });
    expect(browser.error).toBe("ERR_NAME_NOT_RESOLVED");
    gate.channel?.onmessage({
      event: "nav",
      url: "https://two.example/",
      canGoBack: false,
      canGoForward: false,
      loading: true,
    });
    gate.channel?.onmessage({
      event: "load-error",
      code: -3,
      text: "ERR_ABORTED",
      url: "https://two.example/",
    });
    expect(browser.loading).toBe(true);
    expect(browser.error).toBe("ERR_NAME_NOT_RESOLVED");
    expect(browser.alive).toBe(true);
  });

  it("uses empty load-error text as a generic page copy", async () => {
    const gate = await spawnLive();
    gate.channel?.onmessage({
      event: "load-error",
      code: -2,
      text: "",
      url: "https://x.test",
    });
    expect(browser.error).toBe("No se pudo cargar la página");
  });

  it("maps nonzero exits to contract copy", async () => {
    for (const [code, copy] of [
      [10, INCOMPATIBLE],
      [13, INCOMPATIBLE],
      [14, INCOMPATIBLE],
      [15, SANDBOX],
      [16, NO_DISPLAY],
      [99, UNEXPECTED],
    ] as const) {
      const gate = await spawnLive();
      gate.channel?.onmessage({ event: "exit", code });
      expect(browser.alive, `exit ${code} alive`).toBe(false);
      expect(browser.visible, `exit ${code} visible`).toBe(false);
      expect(browser.error, `exit ${code} copy`).toBe(copy);
      await browser.teardown();
    }
  });

  it("does not surface a sandbox-retry fatal while still booting", async () => {
    const gate = installInvoke("hang");
    const pending = browser.spawn();
    expect(browser.booting).toBe(true);
    gate.channel?.onmessage({ event: "fatal", message: SANDBOX, code: 15 });
    expect(browser.booting).toBe(true);
    expect(browser.error).toBeNull();
    gate.channel?.onmessage(ready());
    gate.resolve?.(boot());
    await pending;
    expect(browser.alive).toBe(true);
    expect(browser.error).toBeNull();
  });

  it("does not spawn twice while ADE is retrying", async () => {
    installInvoke("hang");
    const first = browser.spawn();
    const again = browser.spawn();
    expect(tauri.invoke.mock.calls.filter((call) => call[0] === "browser_spawn")).toHaveLength(1);
    await browser.teardown();
    await first;
    await again;
  });

  it("hides the window on ctrl+b without killing", async () => {
    const gate = await spawnLive();
    tauri.invoke.mockClear();
    installInvoke("resolve");
    gate.channel?.onmessage({ event: "shortcut", chord: "ctrl+b" });
    await Promise.resolve();
    await Promise.resolve();
    expect(browser.alive).toBe(true);
    expect(browser.visible).toBe(false);
    expect(tauri.invoke).toHaveBeenCalledWith("browser_set_visible", { visible: false });
    expect(tauri.invoke.mock.calls.map((call) => call[0])).not.toContain("browser_kill");
  });

  it("ignores ctrl+l", async () => {
    const gate = await spawnLive();
    tauri.invoke.mockClear();
    installInvoke("resolve");
    gate.channel?.onmessage({ event: "shortcut", chord: "ctrl+l" });
    await Promise.resolve();
    expect(browser.visible).toBe(true);
    expect(tauri.invoke).not.toHaveBeenCalled();
  });

  it("surfaces a spawn reject as the window error", async () => {
    tauri.invoke.mockImplementation((cmd: string) => {
      if (cmd === "browser_spawn") {
        return Promise.reject(NO_DISPLAY);
      }
      return Promise.resolve();
    });
    await browser.spawn();
    expect(browser.alive).toBe(false);
    expect(browser.visible).toBe(false);
    expect(browser.error).toBe(NO_DISPLAY);
    expect(toasts.notice).toHaveBeenCalledWith(NO_DISPLAY);
  });
});
