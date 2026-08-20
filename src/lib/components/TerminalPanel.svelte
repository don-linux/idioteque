<script lang="ts">
  import { onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { appConfig } from "$lib/app-config.svelte";
  import { isTerminalDockShortcut } from "$lib/terminal-dock";
  import { terminal } from "$lib/terminal.svelte";
  import { xtermFontFamily } from "$lib/terminal-font";
  import "@xterm/xterm/css/xterm.css";

  let { cwd }: { cwd: string } = $props();

  let host: HTMLDivElement | undefined;
  let ready = $state(false);
  let view: Terminal | undefined;
  let fit: FitAddon | undefined;

  function fitAndResize(): void {
    if (!view || !fit || !terminal.open || !host) return;
    if (host.clientWidth < 2 || host.clientHeight < 2) return;

    try {
      fit.fit();
    } catch {
      return;
    }

    terminal.rememberPark(host.clientWidth, host.clientHeight);
    void terminal.resize(view.cols, view.rows);
  }

  onMount(() => {
    if (!host) return;

    const xterm = new Terminal({
      cursorBlink: true,
      fontFamily: xtermFontFamily(appConfig.terminalFontFamily),
      fontSize: appConfig.terminalFontSize,
      theme: {
        background: "#14161a",
        foreground: "#e4e6ea",
        cursor: "#7aa2f7",
        cursorAccent: "#14161a",
        selectionBackground: "#7aa2f722",
      },
    });
    const fitAddon = new FitAddon();

    xterm.loadAddon(fitAddon);
    xterm.attachCustomKeyEventHandler((event) => !isTerminalDockShortcut(event));
    xterm.open(host);
    view = xterm;
    fit = fitAddon;
    terminal.attachWriter((chunk) => xterm.write(chunk));

    const input = xterm.onData((data) => {
      void terminal.write(data);
    });

    const observer = new ResizeObserver(() => {
      if (!terminal.open) return;
      fitAndResize();
    });
    observer.observe(host);
    ready = true;

    return () => {
      view = undefined;
      fit = undefined;
      ready = false;
      observer.disconnect();
      input.dispose();
      terminal.detachWriter();
      xterm.dispose();
    };
  });

  $effect(() => {
    if (!ready || !terminal.open || !cwd) return;

    const root = cwd;
    const shouldSpawn = !terminal.alive && terminal.error === null;

    const frame = requestAnimationFrame(() => {
      fitAndResize();

      if (!shouldSpawn || !view) return;
      void terminal.spawn(root, view.cols, view.rows).then(() => {
        fitAndResize();
        view?.focus();
      });
    });

    return () => {
      cancelAnimationFrame(frame);
    };
  });

  $effect(() => {
    if (!ready || !view) return;

    const family = xtermFontFamily(appConfig.terminalFontFamily);
    const size = appConfig.terminalFontSize;
    const changed =
      view.options.fontFamily !== family || view.options.fontSize !== size;

    if (!changed) return;

    view.options.fontFamily = family;
    view.options.fontSize = size;
    fitAndResize();
  });
</script>

<div class="panel">
  <div class="host" bind:this={host}></div>
</div>

<style>
  .panel {
    display: flex;
    box-sizing: border-box;
    flex: 1;
    flex-direction: column;
    width: 100%;
    min-width: 0;
    height: 100%;
    min-height: 0;
    padding: 0.35rem 0.5rem;
    overflow: hidden;
    background: var(--bg);
  }

  .host {
    flex: 1;
    width: 100%;
    min-width: 0;
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .host :global(.xterm) {
    width: 100%;
    height: 100%;
  }

  .host :global(.xterm-viewport) {
    background-color: var(--bg);
    overflow-y: auto;
  }

  .host :global(.composition-view) {
    background-color: var(--bg);
  }
</style>
