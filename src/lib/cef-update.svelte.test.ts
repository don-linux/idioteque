import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CefUpdateEvent } from "./cef-notices";
import type { NoticeToastOpts } from "./toast";
import type { WorkspaceSurface } from "./workspace-surface";

const { surface, toasts, listenMock, getVersionMock, invokeMock } = vi.hoisted(() => {
  const items: Array<{
    message: string;
    detail?: string;
    action?: { label: string; href: string };
  }> = [];

  return {
    surface: { current: "editor" as WorkspaceSurface },
    toasts: {
      items,
      successLong: vi.fn((message: string) => {
        items.push({ message });
        return items.length;
      }),
      notice: vi.fn((message: string, opts: NoticeToastOpts = {}) => {
        items.push({
          message,
          detail: opts.detail,
          action: opts.action,
        });
        return items.length;
      }),
    },
    listenMock: vi.fn(),
    getVersionMock: vi.fn(),
    invokeMock: vi.fn(),
  };
});

vi.mock("$lib/workspace-surface.svelte", () => ({ surface }));
vi.mock("$lib/toast.svelte", () => ({ toasts }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: getVersionMock }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { CefUpdates } from "./cef-update.svelte";
import { incompatibleDetail, incompatibleMessage, updatedMessage } from "./cef-notices";

const updated: CefUpdateEvent = {
  kind: "updated",
  chromium: "153.0.8000.10",
  cef: "153.0.1+gabc",
};

const incompatible: CefUpdateEvent = {
  kind: "incompatible",
  candidateChromium: "153.0.8000.10",
  candidateCef: "153.0.1+gabc",
  currentChromium: "152.0.7977.83",
  currentCef: "152.0.6+gdef",
  reason: "health-exit-10",
};

function resetToasts(): void {
  toasts.items.length = 0;
  toasts.successLong.mockClear();
  toasts.notice.mockClear();
}

describe("CefUpdates", () => {
  let updates: CefUpdates;

  beforeEach(() => {
    updates = new CefUpdates();
    surface.current = "editor";
    resetToasts();
    listenMock.mockReset();
    getVersionMock.mockReset();
    invokeMock.mockReset();
    getVersionMock.mockRejectedValue(new Error("no tauri"));
    invokeMock.mockRejectedValue(new Error("no tauri"));
  });

  it("does not toast while the browser surface is covering the host window", () => {
    surface.current = "browser";
    updates.enqueue(updated);
    updates.enqueue(incompatible);

    expect(toasts.successLong).not.toHaveBeenCalled();
    expect(toasts.notice).not.toHaveBeenCalled();
  });

  it("flushes the deferred queue only after leaving the browser", () => {
    surface.current = "browser";
    updates.enqueue(updated);
    updates.enqueue(incompatible);

    updates.flush();
    expect(toasts.successLong).not.toHaveBeenCalled();
    expect(toasts.notice).not.toHaveBeenCalled();

    surface.current = "editor";
    updates.flush();

    expect(toasts.successLong).toHaveBeenCalledTimes(1);
    expect(toasts.successLong).toHaveBeenCalledWith(updatedMessage(updated));
    expect(toasts.notice).toHaveBeenCalledTimes(1);
    expect(toasts.notice.mock.calls[0][0]).toBe(incompatibleMessage(incompatible));
    expect(toasts.notice.mock.calls[0][1]).toMatchObject({
      detail: incompatibleDetail(incompatible),
      action: { label: "Abrir issue" },
    });
  });

  it("is a no-op if flush runs again after the queue was drained", () => {
    surface.current = "browser";
    updates.enqueue(updated);
    surface.current = "terminals";
    updates.flush();
    updates.flush();

    expect(toasts.successLong).toHaveBeenCalledTimes(1);
    expect(toasts.items).toHaveLength(1);
  });

  it("does not toast a duplicate updated payload from a second emit", () => {
    updates.enqueue(updated);
    updates.enqueue({ ...updated });

    expect(toasts.successLong).toHaveBeenCalledTimes(1);
  });

  it("does not toast a duplicate after it was already flushed from the browser queue", () => {
    surface.current = "browser";
    updates.enqueue(updated);
    surface.current = "editor";
    updates.flush();
    updates.enqueue({ ...updated });

    expect(toasts.successLong).toHaveBeenCalledTimes(1);
  });

  it("still shows an incompatible notice after an updated toast for the same milestone", () => {
    updates.enqueue(updated);
    updates.enqueue(incompatible);

    expect(toasts.successLong).toHaveBeenCalledTimes(1);
    expect(toasts.notice).toHaveBeenCalledTimes(1);
  });

  it("returns a no-op unlisten when listen fails outside Tauri", async () => {
    listenMock.mockRejectedValue(new Error("not allowed on this platform"));

    const stop = await updates.start();

    expect(typeof stop).toBe("function");
    expect(() => stop()).not.toThrow();
    expect(listenMock).toHaveBeenCalledWith("cef-update", expect.any(Function));
  });

  it("also survives a synchronous listen throw (plain browser preview)", async () => {
    listenMock.mockImplementation(() => {
      throw new Error("window.__TAURI_INTERNALS__ is not defined");
    });

    const stop = await updates.start();
    expect(() => stop()).not.toThrow();
  });

  it("still listens after getVersion and cef_runtime_info fail", async () => {
    let handler: ((event: { payload: CefUpdateEvent }) => void) | undefined;
    listenMock.mockImplementation(async (_name: string, cb: (event: { payload: CefUpdateEvent }) => void) => {
      handler = cb;
      return () => {};
    });

    await updates.start();
    expect(handler).toBeTypeOf("function");

    handler?.({ payload: incompatible });
    const href = toasts.notice.mock.calls[0]?.[1]?.action?.href ?? "";
    const url = new URL(href);

    expect(url.searchParams.get("title")).toBe(
      "CEF 153.0.8000.10 no compatible con idioteque dev",
    );
    expect(url.searchParams.get("body")).toContain("hostApiVersion: 0");
    expect(url.searchParams.get("body")).toContain("plataforma: unknown");
  });

  it("does not attach a second listener if start() races", async () => {
    const unlistens: Array<ReturnType<typeof vi.fn>> = [];
    const handlers: Array<(event: { payload: CefUpdateEvent }) => void> = [];

    listenMock.mockImplementation(async (_name: string, cb: (event: { payload: CefUpdateEvent }) => void) => {
      handlers.push(cb);
      const unlisten = vi.fn();
      unlistens.push(unlisten);
      return unlisten;
    });

    getVersionMock.mockImplementation(() => new Promise((resolve) => setTimeout(() => resolve("0.1.0"), 5)));
    invokeMock.mockResolvedValue({
      hostApiVersion: 15200,
      platform: "linux64",
    });

    const [first, second] = await Promise.all([updates.start(), updates.start()]);
    first();
    second();

    const active = handlers.length - unlistens.filter((fn) => fn.mock.calls.length > 0).length;
    expect(active).toBeLessThanOrEqual(1);

    for (const handler of handlers) {
      handler({ payload: updated });
    }
    expect(toasts.successLong).toHaveBeenCalledTimes(1);
  });

  it("wires a successful listen into enqueue so a browser-surface event stays queued", async () => {
    let handler: ((event: { payload: CefUpdateEvent }) => void) | undefined;
    listenMock.mockImplementation(async (_name: string, cb: (event: { payload: CefUpdateEvent }) => void) => {
      handler = cb;
      return () => {};
    });
    getVersionMock.mockResolvedValue("0.1.0");
    invokeMock.mockResolvedValue({ hostApiVersion: 15200, platform: "linux64" });

    await updates.start();
    surface.current = "browser";
    handler?.({ payload: updated });
    expect(toasts.successLong).not.toHaveBeenCalled();

    surface.current = "editor";
    updates.flush();
    expect(toasts.successLong).toHaveBeenCalledTimes(1);
  });
});
