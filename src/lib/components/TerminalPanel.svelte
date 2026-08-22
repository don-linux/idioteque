<script lang="ts">
  import { onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebglAddon } from "@xterm/addon-webgl";
  import { appConfig } from "$lib/app-config.svelte";
  import { isSaveShortcut } from "$lib/save-shortcut";
  import { isTerminalDockShortcut, isTerminalSurfaceShortcut } from "$lib/terminal-dock";
  import { attachTerminalRenderer } from "$lib/terminal-renderer";
  import { terminal } from "$lib/terminal.svelte";
  import { xtermFontFamily } from "$lib/terminal-font";
  import { TERMINAL_XTERM_OPTIONS, resolveTerminalTheme } from "$lib/terminal-theme";
  import "@xterm/xterm/css/xterm.css";

  let {
    cwd,
    sessionId,
    visible = true,
  }: { cwd: string; sessionId: string; visible?: boolean } = $props();

  let host: HTMLDivElement | undefined;
  let ready = $state(false);
  let view: Terminal | undefined;
  let fit: FitAddon | undefined;
  let lastAppliedThemeId: string | undefined;
  let session = $derived(terminal.session(sessionId));

  function fitAndResize(): void {
    if (!view || !fit || !host || !visible) return;
    if (host.clientWidth < 2 || host.clientHeight < 2) return;

    try {
      fit.fit();
    } catch {
      return;
    }

    if (terminal.surface === "editor") {
      terminal.rememberPark(host.clientWidth, host.clientHeight);
    }

    void terminal.resize(sessionId, view.cols, view.rows);
  }

  onMount(() => {
    if (!host) return;

    const xterm = new Terminal({
      ...TERMINAL_XTERM_OPTIONS,
      fontFamily: xtermFontFamily(appConfig.terminalFontFamily),
      fontSize: appConfig.terminalFontSize,
      theme: { ...resolveTerminalTheme(appConfig.terminalTheme) },
    });
    lastAppliedThemeId = appConfig.terminalTheme;
    const fitAddon = new FitAddon();

    xterm.loadAddon(fitAddon);
    xterm.attachCustomKeyEventHandler((event) => {
      if (isTerminalDockShortcut(event) || isTerminalSurfaceShortcut(event)) return false;
      if (isSaveShortcut(event) && terminal.surface === "editor") return false;
      return true;
    });
    xterm.open(host);
    const renderer = attachTerminalRenderer(xterm, () => new WebglAddon());
    view = xterm;
    fit = fitAddon;
    terminal.attachWriter(sessionId, (chunk) => xterm.write(chunk));

    const input = xterm.onData((data) => {
      void terminal.write(sessionId, data);
    });

    const observer = new ResizeObserver(() => {
      if (!visible) return;
      fitAndResize();
    });
    observer.observe(host);
    ready = true;

    return () => {
      view = undefined;
      fit = undefined;
      lastAppliedThemeId = undefined;
      ready = false;
      observer.disconnect();
      input.dispose();
      terminal.detachWriter(sessionId);
      renderer.dispose();
      xterm.dispose();
    };
  });

  $effect(() => {
    if (!ready || !cwd || !session) return;

    const root = cwd;
    const id = sessionId;
    const shouldSpawn = !session.alive && session.error === null;

    const frame = requestAnimationFrame(() => {
      if (visible) fitAndResize();

      if (!shouldSpawn || !view) return;
      void terminal.spawn(id, root, view.cols, view.rows).then(() => {
        if (visible) fitAndResize();
        if (terminal.activeId === id) view?.focus();
      });
    });

    return () => {
      cancelAnimationFrame(frame);
    };
  });

  $effect(() => {
    if (!ready || !visible) return;
    terminal.surface;
    terminal.open;

    const frame = requestAnimationFrame(() => {
      fitAndResize();
      view?.refresh(0, Math.max(0, (view.rows ?? 1) - 1));
    });

    return () => {
      cancelAnimationFrame(frame);
    };
  });

  $effect(() => {
    if (!ready || !view) return;

    const family = xtermFontFamily(appConfig.terminalFontFamily);
    const size = appConfig.terminalFontSize;
    const themeId = appConfig.terminalTheme;
    const changed = view.options.fontFamily !== family || view.options.fontSize !== size;

    if (themeId !== lastAppliedThemeId) {
      view.options.theme = { ...resolveTerminalTheme(themeId) };
      lastAppliedThemeId = themeId;
    }

    if (!changed) return;

    view.options.fontFamily = family;
    view.options.fontSize = size;
    if (visible) fitAndResize();
  });
</script>

<div class="panel" style:--terminal-bg={resolveTerminalTheme(appConfig.terminalTheme).background}>
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
    background: var(--terminal-bg, var(--bg));
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
    background-color: var(--terminal-bg, var(--bg));
    overflow-y: auto;
  }

  .host :global(.composition-view) {
    background-color: var(--terminal-bg, var(--bg));
  }
</style>
