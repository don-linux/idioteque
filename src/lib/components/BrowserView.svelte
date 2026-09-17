<script lang="ts">
  import { boundsChanged, cssBoundsOf, physicalBounds } from "$lib/browser-bounds";
  import { browser } from "$lib/browser.svelte";
  import BrowserToolbar from "$lib/components/BrowserToolbar.svelte";

  let lastPhysical: ReturnType<typeof physicalBounds> | null = null;
  let lastCss: ReturnType<typeof cssBoundsOf> | null = null;

  function sameCss(a: typeof lastCss, b: NonNullable<typeof lastCss>): boolean {
    return a !== null && a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h;
  }

  function attachHost(node: HTMLElement): () => void {
    let frame = 0;

    function publish(forceSpawn: boolean): void {
      const rect = node.getBoundingClientRect();
      if (rect.width < 1 || rect.height < 1) return;

      const scale = window.devicePixelRatio || 1;
      const css = cssBoundsOf(rect);
      const physical = physicalBounds(rect, scale);

      if (forceSpawn || (!browser.alive && !browser.booting)) {
        lastPhysical = physical;
        lastCss = css;
        void browser.spawn(css, scale);
        return;
      }

      if (!browser.alive) return;
      // Parked off-screen: never push that rect, the real one comes when shown.
      if (!browser.visible) return;
      if (!boundsChanged(lastPhysical, physical) && sameCss(lastCss, css)) return;
      lastPhysical = physical;
      lastCss = css;
      void browser.setBounds(css, scale);
    }

    function schedule(): void {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        publish(browser.pendingSpawn);
      });
    }

    const observer = new ResizeObserver(() => {
      schedule();
    });
    observer.observe(node);
    window.addEventListener("resize", schedule);

    if (!browser.alive && !browser.booting) {
      publish(true);
    } else {
      schedule();
    }

    $effect(() => {
      if (!browser.pendingSpawn) return;
      if (browser.alive || browser.booting) return;
      schedule();
    });

    $effect(() => {
      const visible = browser.visible;
      if (!browser.alive) return;
      if (visible) {
        // The slot was just un-parked: place the hole before showing it.
        lastCss = null;
        publish(false);
      }
      void browser.setVisible(visible);
      if (!visible) return;

      const id = requestAnimationFrame(() => {
        void browser.focus();
      });
      return () => cancelAnimationFrame(id);
    });

    return () => {
      observer.disconnect();
      window.removeEventListener("resize", schedule);
      if (frame) cancelAnimationFrame(frame);
    };
  }

  let placeholder = $derived(
    browser.booting
      ? "Arrancando Chromium…"
      : (browser.error ?? "Arrancando Chromium…"),
  );
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
