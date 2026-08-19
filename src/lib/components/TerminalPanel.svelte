<script lang="ts">
  import { onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { terminal } from "$lib/terminal.svelte";
  import "@xterm/xterm/css/xterm.css";

  let { cwd }: { cwd: string } = $props();

  let host: HTMLDivElement | undefined;
  let ready = $state(false);
  let view: Terminal | undefined;
  let fit: FitAddon | undefined;

  function fitAndResize(): void {
    if (!view || !fit) return;

    try {
      fit.fit();
    } catch {
      return;
    }

    void terminal.resize(view.cols, view.rows);
  }

  onMount(() => {
    if (!host) return;

    const xterm = new Terminal({
      cursorBlink: true,
      fontFamily: '"JetBrains Mono", "SF Mono", ui-monospace, monospace',
      fontSize: 13,
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
</script>

<div class="panel">
  <div class="host" bind:this={host}></div>
</div>

<style>
  .panel {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--bg);
  }

  .host {
    flex: 1;
    min-width: 0;
    min-height: 0;
    padding: 0.35rem 0.5rem;
  }

  .host :global(.xterm) {
    height: 100%;
  }

  .host :global(.xterm-viewport) {
    overflow-y: auto;
  }
</style>
