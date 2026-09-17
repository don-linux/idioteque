import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { transpileModule, ModuleKind, ScriptTarget } from "typescript";

const VIEW = fileURLToPath(new URL("./BrowserView.svelte", import.meta.url));
const SOURCE = readFileSync(VIEW, "utf8");

const module = await loadHostPolicy();

const {
  BOOTING_COPY,
  attachResizeAndWindow,
  createFrameGate,
  cssBoxOf,
  decideHostAction,
  deviceScale,
  dprMediaQuery,
  hostPlaceholder,
  onVisibilityTick,
  sameCss,
  shouldApplyBounds,
  shouldSchedulePendingSpawn,
  usableHostRect,
  watchResolution,
} = module;

describe("usableHostRect (rect 0 / CEF #3396)", () => {
  const shown = { x: 80, y: 40, width: 1200, height: 700 };

  it("accepts a finite box of at least 1×1 CSS px", () => {
    expect(usableHostRect(shown)).toBe(true);
    expect(usableHostRect({ x: 0, y: 0, width: 1, height: 1 })).toBe(true);
    expect(usableHostRect({ x: -12000, y: 0, width: 640, height: 480 })).toBe(
      true,
    );
  });

  it("rejects a collapsed or sub-pixel host (Ozone X11 initial size)", () => {
    expect(usableHostRect({ ...shown, width: 0, height: 700 })).toBe(false);
    expect(usableHostRect({ ...shown, width: 1200, height: 0 })).toBe(false);
    expect(usableHostRect({ ...shown, width: 0.4, height: 700 })).toBe(false);
    expect(usableHostRect({ ...shown, width: 1200, height: 0.9 })).toBe(false);
    expect(usableHostRect({ ...shown, width: -8, height: 700 })).toBe(false);
    expect(usableHostRect({ ...shown, width: 1200, height: -1 })).toBe(false);
  });

  it("rejects NaN, ±Infinity, and non-finite origin", () => {
    expect(usableHostRect({ ...shown, width: Number.NaN })).toBe(false);
    expect(usableHostRect({ ...shown, height: Number.NaN })).toBe(false);
    expect(usableHostRect({ ...shown, x: Number.NaN })).toBe(false);
    expect(usableHostRect({ ...shown, y: Number.POSITIVE_INFINITY })).toBe(
      false,
    );
    expect(usableHostRect({ ...shown, width: Number.POSITIVE_INFINITY })).toBe(
      false,
    );
    expect(usableHostRect({ ...shown, height: Number.NEGATIVE_INFINITY })).toBe(
      false,
    );
  });
});

describe("deviceScale (DPR)", () => {
  it("keeps a positive finite devicePixelRatio", () => {
    expect(deviceScale(1)).toBe(1);
    expect(deviceScale(1.25)).toBe(1.25);
    expect(deviceScale(2)).toBe(2);
    expect(deviceScale(2.75)).toBe(2.75);
  });

  it("falls back to 1 when DPR is 0, negative, or non-finite (keep || 1)", () => {
    expect(deviceScale(0)).toBe(1);
    expect(deviceScale(-1)).toBe(1);
    expect(deviceScale(-0)).toBe(1);
    expect(deviceScale(Number.NaN)).toBe(1);
    expect(deviceScale(Number.POSITIVE_INFINITY)).toBe(1);
    expect(deviceScale(Number.NEGATIVE_INFINITY)).toBe(1);
  });

  it("does not pass a 0 scale through to --idq-scale / browser_spawn", () => {
    expect(deviceScale(0)).not.toBe(0);
  });
});

describe("decideHostAction (park off-screen)", () => {
  const live = {
    usable: true,
    visible: true,
    alive: true,
    booting: false,
    forceSpawn: false,
  };

  it("never pushes the parked slot (left:-12000) even on forceSpawn", () => {
    expect(
      decideHostAction({ ...live, visible: false, forceSpawn: true }),
    ).toBe("skip-park");
    expect(
      decideHostAction({
        ...live,
        visible: false,
        alive: false,
        forceSpawn: true,
      }),
    ).toBe("skip-park");
    expect(decideHostAction({ ...live, visible: false })).toBe("skip-park");
  });

  it("skips a 0×0 rect before spawn so Ozone does not lock a tiny window", () => {
    expect(
      decideHostAction({
        ...live,
        usable: false,
        alive: false,
        forceSpawn: true,
      }),
    ).toBe("skip-rect");
  });

  it("spawns when the real host is visible and the session is idle", () => {
    expect(
      decideHostAction({ ...live, alive: false, forceSpawn: true }),
    ).toBe("spawn");
    expect(decideHostAction({ ...live, alive: false })).toBe("spawn");
  });

  it("does not spawn a second host while booting unless forceSpawn", () => {
    expect(
      decideHostAction({
        ...live,
        alive: false,
        booting: true,
        forceSpawn: false,
      }),
    ).toBe("skip-dead");
    expect(
      decideHostAction({
        ...live,
        alive: false,
        booting: true,
        forceSpawn: true,
      }),
    ).toBe("spawn");
  });

  it("updates bounds only while alive and shown", () => {
    expect(decideHostAction(live)).toBe("bounds");
  });
});

describe("shouldApplyBounds (DPR without a CSS resize)", () => {
  const css = { x: 10, y: 20, w: 100, h: 50 };
  const physical = { x: 10, y: 20, w: 100, h: 50 };

  it("is a no-op when CSS, physical box, and scale match", () => {
    expect(
      shouldApplyBounds(physical, physical, css, css, 1, 1),
    ).toBe(false);
  });

  it("pushes when only the scale changes (monitor / OS zoom)", () => {
    expect(
      shouldApplyBounds(physical, physical, css, css, 1, 2),
    ).toBe(true);
  });

  it("pushes when the CSS box moved even if physical rounded the same", () => {
    expect(
      shouldApplyBounds(
        physical,
        physical,
        css,
        { ...css, x: 10.4 },
        1,
        1,
      ),
    ).toBe(true);
  });

  it("pushes after un-park (forgotten last box)", () => {
    expect(shouldApplyBounds(null, physical, null, css, null, 1)).toBe(true);
  });
});

describe("createFrameGate (ResizeObserver + resize coalesce)", () => {
  it("coalesces observer and window.resize into one rAF", () => {
    const clock = fakeClock();
    const gate = createFrameGate(clock);
    let runs = 0;
    gate.schedule(() => {
      runs += 1;
    });
    gate.schedule(() => {
      runs += 1;
    });
    expect(clock.queued()).toBe(1);
    clock.flush();
    expect(runs).toBe(1);
  });

  it("can schedule again after the frame fires", () => {
    const clock = fakeClock();
    const gate = createFrameGate(clock);
    let runs = 0;
    gate.schedule(() => {
      runs += 1;
    });
    clock.flush();
    gate.schedule(() => {
      runs += 1;
    });
    clock.flush();
    expect(runs).toBe(2);
  });

  it("cancel drops a pending frame (destroy mid-rAF)", () => {
    const clock = fakeClock();
    const gate = createFrameGate(clock);
    let runs = 0;
    gate.schedule(() => {
      runs += 1;
    });
    gate.cancel();
    clock.flush();
    expect(runs).toBe(0);
  });
});

describe("attachResizeAndWindow (ResizeObserver)", () => {
  it("observes the host node and disconnects on dispose", () => {
    const ro = fakeResizeObserver();
    const windowEvents: Array<{ type: string; add: boolean }> = [];
    const stop = attachResizeAndWindow(ro.node, () => {}, {
      ResizeObserver: ro.Ctor,
      addEventListener: (type) => {
        windowEvents.push({ type, add: true });
      },
      removeEventListener: (type) => {
        windowEvents.push({ type, add: false });
      },
      devicePixelRatio: 1,
    });

    expect(ro.observed).toEqual([ro.node]);
    expect(ro.disconnected).toBe(false);
    expect(windowEvents).toEqual([{ type: "resize", add: true }]);

    stop();
    expect(ro.disconnected).toBe(true);
    expect(windowEvents).toEqual([
      { type: "resize", add: true },
      { type: "resize", add: false },
    ]);
  });

  it("schedules from ResizeObserver, window.resize, and a DPR media change", () => {
    const ro = fakeResizeObserver();
    let resizeListener: (() => void) | null = null;
    let scheduled = 0;
    const dpr = fakeDprWatch();
    const stop = attachResizeAndWindow(ro.node, () => {
      scheduled += 1;
    }, {
      ResizeObserver: ro.Ctor,
      addEventListener: (_type, listener) => {
        resizeListener = listener;
      },
      removeEventListener: () => {
        resizeListener = null;
      },
      devicePixelRatio: 1,
      watchResolution: dpr.watch,
    });

    ro.fire();
    resizeListener?.();
    dpr.fire();
    expect(scheduled).toBe(3);

    stop();
    expect(dpr.stopped).toBe(true);
    ro.fire();
    dpr.fire();
    expect(scheduled).toBe(3);
  });
});

describe("watchResolution (DPR listener)", () => {
  it("re-arms the resolution query when DPR changes", () => {
    let dpr = 1;
    const media = fakeMatchMedia();
    let ticks = 0;
    const stop = watchResolution(
      () => dpr,
      (query) => media.match(query),
      () => {
        ticks += 1;
      },
    );

    expect(media.queries).toEqual(["(resolution: 1dppx)"]);
    dpr = 2;
    media.change(0);
    expect(ticks).toBe(1);
    expect(media.queries).toEqual([
      "(resolution: 1dppx)",
      "(resolution: 2dppx)",
    ]);

    stop();
    media.change(1);
    expect(ticks).toBe(1);
  });

  it("is a no-op when matchMedia throws (no jsdom / odd hosts)", () => {
    expect(() =>
      watchResolution(
        () => 1,
        () => {
          throw new Error("matchMedia missing");
        },
        () => {
          throw new Error("should not run");
        },
      )(),
    ).not.toThrow();
  });

  it("builds the official resolution media query", () => {
    expect(dprMediaQuery(1)).toBe("(resolution: 1dppx)");
    expect(dprMediaQuery(2.5)).toBe("(resolution: 2.5dppx)");
  });
});

describe("pendingSpawn and visibility ticks", () => {
  it("schedules a spawn only when pending and idle", () => {
    expect(
      shouldSchedulePendingSpawn({
        pendingSpawn: true,
        alive: false,
        booting: false,
      }),
    ).toBe(true);
    expect(
      shouldSchedulePendingSpawn({
        pendingSpawn: true,
        alive: false,
        booting: true,
      }),
    ).toBe(false);
    expect(
      shouldSchedulePendingSpawn({
        pendingSpawn: true,
        alive: true,
        booting: false,
      }),
    ).toBe(false);
    expect(
      shouldSchedulePendingSpawn({
        pendingSpawn: false,
        alive: false,
        booting: false,
      }),
    ).toBe(false);
  });

  it("on show: forget last CSS, publish, setVisible, focus — only while alive", () => {
    expect(onVisibilityTick({ visible: true, alive: true })).toEqual({
      forgetCss: true,
      publish: true,
      setVisible: true,
      focusAfterFrame: true,
    });
  });

  it("on hide: setVisible only — never publish the parked rect", () => {
    expect(onVisibilityTick({ visible: false, alive: true })).toEqual({
      forgetCss: false,
      publish: false,
      setVisible: true,
      focusAfterFrame: false,
    });
  });

  it("does not auto-respawn when the host dies while still shown", () => {
    expect(onVisibilityTick({ visible: true, alive: false })).toEqual({
      forgetCss: false,
      publish: false,
      setVisible: false,
      focusAfterFrame: false,
    });
  });
});

describe("hostPlaceholder", () => {
  it("keeps Arrancando while booting even if a fatal leaked", () => {
    expect(hostPlaceholder(true, "fatal 15")).toBe(BOOTING_COPY);
    expect(hostPlaceholder(true, null)).toBe("Arrancando Chromium…");
  });

  it("shows the error once booting ends, else the same Arrancando copy", () => {
    expect(hostPlaceholder(false, "El navegador necesita X11")).toBe(
      "El navegador necesita X11",
    );
    expect(hostPlaceholder(false, null)).toBe(BOOTING_COPY);
  });
});

describe("cssBoxOf / sameCss", () => {
  it("renames width/height without scaling", () => {
    expect(cssBoxOf({ x: 1, y: 2, width: 3, height: 4 })).toEqual({
      x: 1,
      y: 2,
      w: 3,
      h: 4,
    });
  });

  it("treats a missing last box as different", () => {
    const box = { x: 1, y: 2, w: 3, h: 4 };
    expect(sameCss(null, box)).toBe(false);
    expect(sameCss(box, box)).toBe(true);
    expect(sameCss({ ...box, w: 9 }, box)).toBe(false);
  });
});

describe("BrowserView.svelte wiring", () => {
  it("exports the policy from a script module and attaches it to .host", () => {
    expect(SOURCE).toContain('<script module lang="ts">');
    expect(SOURCE).toContain("{@attach attachHost}");
    expect(SOURCE).toContain('class="host"');
    expect(SOURCE).toMatch(/untrack\s*\(/);
    expect(SOURCE).toContain("ResizeObserver");
    expect(SOURCE).toContain("Parked off-screen");
  });

  it("uses the extracted decisions instead of a parallel attach copy", () => {
    const instance = SOURCE.split(/<script lang="ts">/)[1] ?? "";
    for (const name of [
      "usableHostRect",
      "deviceScale",
      "decideHostAction",
      "shouldApplyBounds",
      "createFrameGate",
      "attachResizeAndWindow",
      "watchResolution",
      "shouldSchedulePendingSpawn",
      "onVisibilityTick",
      "hostPlaceholder",
    ]) {
      expect(instance, name).toContain(name);
    }
  });

  it("does not hardcode the workspace park offset (that CSS lives on +page)", () => {
    expect(SOURCE).not.toContain("-12000");
  });
});

type HostPolicy = {
  BOOTING_COPY: string;
  attachResizeAndWindow: (
    node: unknown,
    schedule: () => void,
    deps: {
      ResizeObserver: new (cb: () => void) => {
        observe: (target: unknown) => void;
        disconnect: () => void;
      };
      addEventListener: (type: "resize", listener: () => void) => void;
      removeEventListener: (type: "resize", listener: () => void) => void;
      devicePixelRatio: number;
      watchResolution?: (dpr: number, onChange: () => void) => () => void;
    },
  ) => () => void;
  createFrameGate: (clock: {
    requestAnimationFrame: (cb: () => void) => number;
    cancelAnimationFrame: (id: number) => void;
  }) => {
    schedule: (run: () => void) => void;
    cancel: () => void;
    pending: () => boolean;
  };
  cssBoxOf: (rect: {
    x: number;
    y: number;
    width: number;
    height: number;
  }) => { x: number; y: number; w: number; h: number };
  decideHostAction: (session: {
    usable: boolean;
    visible: boolean;
    alive: boolean;
    booting: boolean;
    forceSpawn: boolean;
  }) => string;
  deviceScale: (dpr: number) => number;
  dprMediaQuery: (dpr: number) => string;
  hostPlaceholder: (booting: boolean, error: string | null) => string;
  onVisibilityTick: (session: { visible: boolean; alive: boolean }) => {
    forgetCss: boolean;
    publish: boolean;
    setVisible: boolean;
    focusAfterFrame: boolean;
  };
  sameCss: (
    a: { x: number; y: number; w: number; h: number } | null,
    b: { x: number; y: number; w: number; h: number },
  ) => boolean;
  shouldApplyBounds: (
    lastPhysical: { x: number; y: number; w: number; h: number } | null,
    nextPhysical: { x: number; y: number; w: number; h: number },
    lastCss: { x: number; y: number; w: number; h: number } | null,
    nextCss: { x: number; y: number; w: number; h: number },
    lastScale: number | null,
    nextScale: number,
  ) => boolean;
  shouldSchedulePendingSpawn: (session: {
    pendingSpawn: boolean;
    alive: boolean;
    booting: boolean;
  }) => boolean;
  usableHostRect: (rect: {
    x: number;
    y: number;
    width: number;
    height: number;
  }) => boolean;
  watchResolution: (
    readDpr: () => number,
    matchMedia: (query: string) => {
      addEventListener: (type: "change", listener: () => void) => void;
      removeEventListener: (type: "change", listener: () => void) => void;
    },
    onChange: () => void,
  ) => () => void;
};

async function loadHostPolicy(): Promise<HostPolicy> {
  const match = SOURCE.match(/<script module lang="ts">\n([\s\S]*?)\n<\/script>/);
  if (!match) {
    throw new Error(
      "BrowserView.svelte must export the host policy from <script module lang=\"ts\">",
    );
  }
  if (/\bimport\b/.test(match[1])) {
    throw new Error("script module must stay import-free so vitest can load it");
  }
  const { outputText } = transpileModule(match[1], {
    compilerOptions: {
      module: ModuleKind.ESNext,
      target: ScriptTarget.ES2022,
    },
    fileName: "BrowserView.module.ts",
  });
  const dir = mkdtempSync(join(tmpdir(), "browser-view-"));
  const file = join(dir, "policy.mjs");
  writeFileSync(file, outputText);
  return import(pathToFileURL(file).href) as Promise<HostPolicy>;
}

function fakeClock() {
  let nextId = 1;
  const pending = new Map<number, () => void>();
  return {
    requestAnimationFrame(cb: () => void) {
      const id = nextId++;
      pending.set(id, cb);
      return id;
    },
    cancelAnimationFrame(id: number) {
      pending.delete(id);
    },
    queued() {
      return pending.size;
    },
    flush() {
      for (const [id, cb] of [...pending]) {
        pending.delete(id);
        cb();
      }
    },
  };
}

function fakeResizeObserver() {
  const node = { id: "host" };
  let callback: (() => void) | null = null;
  let disconnected = false;
  const observed: unknown[] = [];
  class Ctor {
    constructor(cb: () => void) {
      callback = cb;
    }
    observe(target: unknown) {
      observed.push(target);
    }
    disconnect() {
      disconnected = true;
      callback = null;
    }
  }
  return {
    node,
    observed,
    get disconnected() {
      return disconnected;
    },
    Ctor,
    fire() {
      callback?.();
    },
  };
}

function fakeDprWatch() {
  let listener: (() => void) | null = null;
  let stopped = false;
  return {
    get stopped() {
      return stopped;
    },
    watch(_dpr: number, onChange: () => void) {
      listener = onChange;
      return () => {
        stopped = true;
        listener = null;
      };
    },
    fire() {
      listener?.();
    },
  };
}

function fakeMatchMedia() {
  const queries: string[] = [];
  const listeners: Array<() => void> = [];
  return {
    queries,
    match(query: string) {
      queries.push(query);
      const index = listeners.length;
      return {
        addEventListener(_type: "change", listener: () => void) {
          listeners[index] = listener;
        },
        removeEventListener(_type: "change", listener: () => void) {
          if (listeners[index] === listener) listeners[index] = () => {};
        },
      };
    },
    change(index: number) {
      listeners[index]?.();
    },
  };
}
