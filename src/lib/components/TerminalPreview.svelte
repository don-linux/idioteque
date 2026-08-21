<script lang="ts">
  import { untrack } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { xtermFontFamily } from "$lib/terminal-font";
  import {
    TERMINAL_PREVIEW_PROMPT,
    TERMINAL_PREVIEW_ROWS,
    fitTerminalPreview,
    terminalPreviewHostHeight,
  } from "$lib/terminal-preview";
  import { TERMINAL_XTERM_OPTIONS, resolveTerminalTheme } from "$lib/terminal-theme";
  import "@xterm/xterm/css/xterm.css";

  let {
    themeId,
    fontSize,
    fontFamily,
  }: {
    themeId: string;
    fontSize: number;
    fontFamily: string | null;
  } = $props();

  let family = $derived(xtermFontFamily(fontFamily));
  let theme = $derived(resolveTerminalTheme(themeId));
  let hostHeight = $derived(terminalPreviewHostHeight(fontSize));

  function attachPreview(node: HTMLElement): () => void {
    const xterm = new Terminal({
      ...TERMINAL_XTERM_OPTIONS,
      disableStdin: true,
      scrollback: 0,
      rows: TERMINAL_PREVIEW_ROWS,
      fontFamily: untrack(() => family),
      fontSize: untrack(() => fontSize),
      theme: { ...untrack(() => theme) },
    });
    const fitAddon = new FitAddon();

    xterm.loadAddon(fitAddon);
    xterm.open(node);
    xterm.write(TERMINAL_PREVIEW_PROMPT);

    const observer = new ResizeObserver(() => {
      fitTerminalPreview(xterm, fitAddon, node);
    });
    observer.observe(node);

    $effect(() => {
      xterm.options.fontFamily = family;
      xterm.options.fontSize = fontSize;
      fitTerminalPreview(xterm, fitAddon, node);
    });

    return () => {
      observer.disconnect();
      xterm.dispose();
    };
  }
</script>

<div
  class="preview-term"
  style:--terminal-bg={theme.background}
  style:--preview-height="{hostHeight}px"
  aria-hidden="true"
>
  {#key themeId}
    <div class="host" {@attach attachPreview}></div>
  {/key}
</div>

<style>
  .preview-term {
    min-width: 0;
    pointer-events: none;
    overflow: hidden;
    background: var(--terminal-bg, var(--bg));
  }

  .host {
    width: 100%;
    height: var(--preview-height);
    min-width: 0;
    overflow: hidden;
  }

  .host :global(.xterm) {
    width: 100%;
    height: 100%;
  }

  .host :global(.xterm-viewport) {
    background-color: var(--terminal-bg, var(--bg));
    overflow: hidden;
  }

  .host :global(.composition-view) {
    background-color: var(--terminal-bg, var(--bg));
  }

  .host :global(.xterm-helper-textarea) {
    display: none;
  }
</style>
