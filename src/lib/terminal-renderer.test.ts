import { describe, expect, it, vi } from "vitest";
import { attachTerminalRenderer, type WebglAddonLike } from "./terminal-renderer";

function fakeWebgl(overrides: Partial<WebglAddonLike> = {}): WebglAddonLike {
  return {
    dispose: vi.fn(),
    activate: vi.fn(),
    onContextLoss: vi.fn(() => ({ dispose: vi.fn() })),
    ...overrides,
  };
}

describe("attachTerminalRenderer", () => {
  it("loads WebGL when the addon can start", () => {
    const addon = fakeWebgl();
    const loadAddon = vi.fn();

    const handle = attachTerminalRenderer({ loadAddon }, () => addon);

    expect(handle.kind).toBe("webgl");
    expect(loadAddon).toHaveBeenCalledWith(addon);
    expect(addon.onContextLoss).toHaveBeenCalledOnce();
  });

  it("falls back to DOM when creating the addon throws", () => {
    const loadAddon = vi.fn();

    const handle = attachTerminalRenderer({ loadAddon }, () => {
      throw new Error("no webgl");
    });

    expect(handle.kind).toBe("dom");
    expect(loadAddon).not.toHaveBeenCalled();
    handle.dispose();
  });

  it("falls back to DOM when loadAddon throws", () => {
    const addon = fakeWebgl();
    const loadAddon = vi.fn(() => {
      throw new Error("attach failed");
    });

    const handle = attachTerminalRenderer({ loadAddon }, () => addon);

    expect(handle.kind).toBe("dom");
    expect(addon.dispose).toHaveBeenCalledOnce();
    handle.dispose();
  });

  it("disposes the addon if the WebGL context is lost", () => {
    let onLoss: (() => void) | undefined;
    const addon = fakeWebgl({
      onContextLoss: vi.fn((listener: () => void) => {
        onLoss = listener;
        return { dispose: vi.fn() };
      }),
    });

    attachTerminalRenderer({ loadAddon: vi.fn() }, () => addon);
    expect(onLoss).toBeTypeOf("function");
    onLoss?.();
    expect(addon.dispose).toHaveBeenCalledOnce();
  });

  it("disposes the addon and the context-loss listener", () => {
    const lossDispose = vi.fn();
    const addon = fakeWebgl({
      onContextLoss: vi.fn(() => ({ dispose: lossDispose })),
    });

    const handle = attachTerminalRenderer({ loadAddon: vi.fn() }, () => addon);
    handle.dispose();

    expect(lossDispose).toHaveBeenCalledOnce();
    expect(addon.dispose).toHaveBeenCalledOnce();
  });
});
