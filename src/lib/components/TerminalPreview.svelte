<script lang="ts">
  import { untrack } from "svelte";
  import { FitAddon } from "@xterm/addon-fit";
  import { Terminal } from "@xterm/xterm";
  import {
    TERMINAL_PREVIEW_BUFFER,
    TERMINAL_PREVIEW_OPTIONS,
    TERMINAL_PREVIEW_ROWS,
    terminalPreviewHostHeight,
  } from "$lib/terminal-preview";
  import { xtermFontFamily } from "$lib/terminal-font";
  import { TERMINAL_XTERM_OPTIONS, resolveTerminalTheme } from "$lib/terminal-theme";
  import "@xterm/xterm/css/xterm.css";

  let {
    themeId,
    fontFamily,
    fontSize,
  }: {
    themeId: string;
    fontFamily: string | null;
    fontSize: number;
  } = $props();

  let hostHeight = $derived(terminalPreviewHostHeight(fontSize));
  let previewBackground = $derived(resolveTerminalTheme(themeId).background);

  function attachPreview(id: string) {
    return (node: HTMLElement) => {
      const term = new Terminal({
        ...TERMINAL_XTERM_OPTIONS,
        ...TERMINAL_PREVIEW_OPTIONS,
        rows: TERMINAL_PREVIEW_ROWS,
        fontFamily: untrack(() => xtermFontFamily(fontFamily)),
        fontSize: untrack(() => fontSize),
        theme: { ...resolveTerminalTheme(id) },
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      term.open(node);

      const textarea = node.querySelector("textarea");
      textarea?.setAttribute("tabindex", "-1");
      textarea?.setAttribute("aria-hidden", "true");

      function resizePreview(): void {
        if (node.clientWidth < 2 || node.clientHeight < 2) return;

        try {
          const proposed = fit.proposeDimensions();
          if (!proposed?.cols) return;
          term.resize(Math.max(proposed.cols, 1), TERMINAL_PREVIEW_ROWS);
        } catch {
          return;
        }
      }

      resizePreview();
      term.write(TERMINAL_PREVIEW_BUFFER);

      const observer = new ResizeObserver(() => {
        resizePreview();
      });
      observer.observe(node);

      $effect(() => {
        term.options.fontFamily = xtermFontFamily(fontFamily);
        term.options.fontSize = fontSize;
        resizePreview();
      });

      return () => {
        observer.disconnect();
        term.dispose();
      };
    };
  }
</script>

<div
  class="host"
  style:height={`${hostHeight}px`}
  style:--terminal-bg={previewBackground}
  aria-hidden="true"
  {@attach attachPreview(themeId)}
></div>

<style>
  .host {
    width: 100%;
    min-width: 0;
    overflow: hidden;
    pointer-events: none;
    background: var(--terminal-bg, transparent);
  }

  .host :global(.xterm) {
    width: 100%;
  }

  .host :global(.xterm-viewport) {
    background-color: var(--terminal-bg, transparent);
    overflow: hidden !important;
  }

  .host :global(.composition-view) {
    background-color: var(--terminal-bg, transparent);
  }

  .host :global(textarea) {
    pointer-events: none;
  }
</style>
