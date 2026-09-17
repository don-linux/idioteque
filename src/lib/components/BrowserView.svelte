<script module lang="ts">
  export const BOOTING_COPY = "Arrancando Chromium…";

  export type HostRect = { x: number; y: number; width: number; height: number };
  export type CssBox = { x: number; y: number; w: number; h: number };
  export type PhysicalBox = { x: number; y: number; w: number; h: number };
  export type HostAction = "skip-rect" | "skip-park" | "skip-dead" | "spawn" | "bounds";

  export function hostPlaceholder(booting: boolean, error: string | null): string {
    return booting ? BOOTING_COPY : (error ?? BOOTING_COPY);
  }

  /** CEF #3396: Ozone X11 can lock a 0-size embed. Skip until the host is ≥ 1×1. */
  export function usableHostRect(rect: HostRect): boolean {
    return (
      Number.isFinite(rect.x) &&
      Number.isFinite(rect.y) &&
      Number.isFinite(rect.width) &&
      Number.isFinite(rect.height) &&
      rect.width >= 1 &&
      rect.height >= 1
    );
  }

  /** Keep `devicePixelRatio || 1`, and never hand 0 / NaN / negative scale to --idq-scale. */
  export function deviceScale(dpr: number): number {
    if (typeof dpr !== "number" || !Number.isFinite(dpr) || dpr <= 0) return 1;
    return dpr;
  }

  export function cssBoxOf(rect: HostRect): CssBox {
    return { x: rect.x, y: rect.y, w: rect.width, h: rect.height };
  }

  export function sameCss(a: CssBox | null, b: CssBox): boolean {
    return a !== null && a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h;
  }

  export function decideHostAction(session: {
    usable: boolean;
    visible: boolean;
    alive: boolean;
    booting: boolean;
    forceSpawn: boolean;
  }): HostAction {
    if (!session.usable) return "skip-rect";
    // Parked off-screen: never push that rect, the real one comes when shown.
    if (!session.visible) return "skip-park";
    if (session.forceSpawn || (!session.alive && !session.booting)) return "spawn";
    if (!session.alive) return "skip-dead";
    return "bounds";
  }

  export function shouldApplyBounds(
    lastPhysical: PhysicalBox | null,
    nextPhysical: PhysicalBox,
    lastCss: CssBox | null,
    nextCss: CssBox,
    lastScale: number | null,
    nextScale: number,
  ): boolean {
    if (lastScale !== nextScale) return true;
    if (lastPhysical === null) return true;
    if (
      lastPhysical.x !== nextPhysical.x ||
      lastPhysical.y !== nextPhysical.y ||
      lastPhysical.w !== nextPhysical.w ||
      lastPhysical.h !== nextPhysical.h
    ) {
      return true;
    }
    return !sameCss(lastCss, nextCss);
  }

  export function shouldSchedulePendingSpawn(session: {
    pendingSpawn: boolean;
    alive: boolean;
    booting: boolean;
  }): boolean {
    return session.pendingSpawn && !session.alive && !session.booting;
  }

  export function onVisibilityTick(session: { visible: boolean; alive: boolean }): {
    forgetCss: boolean;
    publish: boolean;
    setVisible: boolean;
    focusAfterFrame: boolean;
  } {
    if (!session.alive) {
      return {
        forgetCss: false,
        publish: false,
        setVisible: false,
        focusAfterFrame: false,
      };
    }
    if (session.visible) {
      return {
        forgetCss: true,
        publish: true,
        setVisible: true,
        focusAfterFrame: true,
      };
    }
    return {
      forgetCss: false,
      publish: false,
      setVisible: true,
      focusAfterFrame: false,
    };
  }

  export function createFrameGate(clock: {
    requestAnimationFrame: (cb: () => void) => number;
    cancelAnimationFrame: (id: number) => void;
  }): {
    schedule: (run: () => void) => void;
    cancel: () => void;
    pending: () => boolean;
  } {
    let frame = 0;
    return {
      schedule(run: () => void): void {
        if (frame) return;
        frame = clock.requestAnimationFrame(() => {
          frame = 0;
          run();
        });
      },
      cancel(): void {
        if (frame) clock.cancelAnimationFrame(frame);
        frame = 0;
      },
      pending(): boolean {
        return frame !== 0;
      },
    };
  }

  export function dprMediaQuery(dpr: number): string {
    return `(resolution: ${dpr}dppx)`;
  }

  export function watchResolution(
    readDpr: () => number,
    matchMedia: (query: string) => {
      addEventListener: (type: "change", listener: () => void) => void;
      removeEventListener: (type: "change", listener: () => void) => void;
    },
    onChange: () => void,
  ): () => void {
    let media: ReturnType<typeof matchMedia> | null = null;
    let listener: (() => void) | null = null;

    function disarm(): void {
      if (media && listener) media.removeEventListener("change", listener);
      media = null;
      listener = null;
    }

    function arm(): void {
      disarm();
      try {
        media = matchMedia(dprMediaQuery(readDpr()));
      } catch {
        media = null;
        listener = null;
        return;
      }
      listener = () => {
        onChange();
        arm();
      };
      media.addEventListener("change", listener);
    }

    arm();
    return disarm;
  }

  export function attachResizeAndWindow(
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
  ): () => void {
    const observer = new deps.ResizeObserver(() => {
      schedule();
    });
    observer.observe(node);
    deps.addEventListener("resize", schedule);
    const stopDpr = deps.watchResolution?.(deps.devicePixelRatio, schedule) ?? (() => {});
    return () => {
      observer.disconnect();
      deps.removeEventListener("resize", schedule);
      stopDpr();
    };
  }
</script>

<script lang="ts">
  import { untrack } from "svelte";
  import { cssBoundsOf, physicalBounds } from "$lib/browser-bounds";
  import { browser, shouldGiftCefFocus } from "$lib/browser.svelte";
  import BrowserToolbar from "$lib/components/BrowserToolbar.svelte";

  function attachHost(node: HTMLElement): () => void {
    let lastPhysical: ReturnType<typeof physicalBounds> | null = null;
    let lastCss: ReturnType<typeof cssBoundsOf> | null = null;
    let lastScale: number | null = null;

    function publish(forceSpawn: boolean): void {
      const rect = node.getBoundingClientRect();
      const action = decideHostAction({
        usable: usableHostRect(rect),
        visible: browser.visible,
        alive: browser.alive,
        booting: browser.booting,
        forceSpawn,
      });
      if (action === "skip-rect" || action === "skip-park" || action === "skip-dead") {
        return;
      }

      const scale = deviceScale(window.devicePixelRatio);
      const css = cssBoundsOf(rect);
      const physical = physicalBounds(rect, scale);

      if (action === "spawn") {
        lastPhysical = physical;
        lastCss = css;
        lastScale = scale;
        void browser.spawn(css, scale);
        return;
      }

      if (!shouldApplyBounds(lastPhysical, physical, lastCss, css, lastScale, scale)) {
        return;
      }
      lastPhysical = physical;
      lastCss = css;
      lastScale = scale;
      void browser.setBounds(css, scale);
    }

    const frames = createFrameGate({
      requestAnimationFrame: (cb) => requestAnimationFrame(() => cb()),
      cancelAnimationFrame,
    });

    function schedule(): void {
      frames.schedule(() => {
        publish(browser.pendingSpawn);
      });
    }

    const stopObserve = attachResizeAndWindow(node, schedule, {
      ResizeObserver,
      addEventListener: (type, listener) => window.addEventListener(type, listener),
      removeEventListener: (type, listener) => window.removeEventListener(type, listener),
      devicePixelRatio: window.devicePixelRatio,
      watchResolution: (_dpr, onChange) =>
        watchResolution(() => window.devicePixelRatio, (query) => window.matchMedia(query), onChange),
    });

    untrack(() => {
      if (!browser.alive && !browser.booting) publish(true);
      else schedule();
    });

    $effect(() => {
      if (!shouldSchedulePendingSpawn(browser)) return;
      schedule();
    });

    $effect(() => {
      const tick = onVisibilityTick({
        visible: browser.visible,
        alive: browser.alive,
      });
      if (tick.forgetCss) {
        lastCss = null;
        lastPhysical = null;
        lastScale = null;
      }
      if (tick.publish) publish(false);
      if (tick.setVisible) void browser.setVisible(browser.visible);
      if (!tick.focusAfterFrame) return;
      const id = requestAnimationFrame(() => {
        if (!shouldGiftCefFocus(browser.focusOwner)) return;
        void browser.focus();
      });
      return () => cancelAnimationFrame(id);
    });

    return () => {
      stopObserve();
      frames.cancel();
    };
  }

  let placeholder = $derived(hostPlaceholder(browser.booting, browser.error));
</script>

<div class="view">
  <BrowserToolbar />
  <div class="host" {@attach attachHost}>
    {#if !browser.alive}
      <p class="placeholder">{placeholder}</p>
    {/if}
  </div>
</div>

<style>
  .view {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
  }

  .host {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    background: var(--bg);
  }

  .placeholder {
    display: grid;
    place-content: center;
    width: 100%;
    height: 100%;
    margin: 0;
    color: var(--text-faint);
    font-size: 0.82rem;
  }
</style>
