<script lang="ts">
  import { onMount } from "svelte";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import FontCombobox from "$lib/components/FontCombobox.svelte";
  import { appConfig } from "$lib/app-config.svelte";
  import {
    MAX_TERMINAL_FONT_SIZE,
    MIN_TERMINAL_FONT_SIZE,
    TERMINAL_FONT_PREVIEW,
    clampFontSize,
    xtermFontFamily,
  } from "$lib/terminal-font";

  let previewFamily = $derived(xtermFontFamily(appConfig.terminalFontFamily));

  onMount(() => {
    void appConfig.listSystemFonts();
  });

  function saveFont(family: string | null): void {
    void appConfig.saveTerminal({
      fontFamily: family,
      fontSize: appConfig.terminalFontSize,
    });
  }

  function saveSize(size: number): void {
    void appConfig.saveTerminal({
      fontFamily: appConfig.terminalFontFamily,
      fontSize: clampFontSize(size),
    });
  }

  function onSizeChange(event: Event): void {
    const next = clampFontSize(Number((event.currentTarget as HTMLInputElement).value));
    (event.currentTarget as HTMLInputElement).value = String(next);
    saveSize(next);
  }
</script>

<svelte:head>
  <title>configuración — idioteque</title>
</svelte:head>

<main class="settings">
  <header>
    <a href="/" class="back" aria-label="Volver">
      <ArrowLeft size={18} strokeWidth={1.75} aria-hidden="true" />
    </a>
    <h1>Configuración</h1>
  </header>

  <section class="section" aria-labelledby="terminal-heading">
    <h2 id="terminal-heading">Terminal</h2>
    <p class="lead">
      Fuente y tamaño del panel integrado. Se guardan solos y se aplican al volver.
    </p>

    <div class="field">
      <label for="terminal-font">Fuente</label>
      <FontCombobox
        id="terminal-font"
        fonts={appConfig.fonts}
        value={appConfig.terminalFontFamily}
        disabled={!appConfig.fontsLoaded}
        onSelect={saveFont}
      />
      {#if !appConfig.fontsLoaded}
        <p class="hint">Leyendo las fuentes del sistema…</p>
      {/if}
    </div>

    <div class="field">
      <label for="terminal-font-size">Tamaño</label>
      <div class="stepper">
        <button
          type="button"
          aria-label="Reducir tamaño"
          disabled={appConfig.terminalFontSize <= MIN_TERMINAL_FONT_SIZE}
          onclick={() => saveSize(appConfig.terminalFontSize - 1)}
        >
          −
        </button>
        <input
          id="terminal-font-size"
          type="number"
          min={MIN_TERMINAL_FONT_SIZE}
          max={MAX_TERMINAL_FONT_SIZE}
          value={appConfig.terminalFontSize}
          onchange={onSizeChange}
        />
        <button
          type="button"
          aria-label="Aumentar tamaño"
          disabled={appConfig.terminalFontSize >= MAX_TERMINAL_FONT_SIZE}
          onclick={() => saveSize(appConfig.terminalFontSize + 1)}
        >
          +
        </button>
        <span class="unit">px</span>
      </div>
    </div>

    <p
      class="preview"
      style:font-family={previewFamily}
      style:font-size={`${appConfig.terminalFontSize}px`}
    >
      {TERMINAL_FONT_PREVIEW}
    </p>

    {#if appConfig.error}
      <p class="error">{appConfig.error}</p>
    {/if}
  </section>
</main>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    gap: 2rem;
    height: 100%;
    overflow: auto;
    padding: 2rem 2rem 3rem;
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  h1 {
    margin: 0;
    font-size: 1.6rem;
    font-weight: 600;
  }

  h2 {
    margin: 0 0 0.35rem;
    font-size: 1.05rem;
    font-weight: 600;
  }

  .back {
    display: inline-flex;
    box-sizing: border-box;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    width: 2.35rem;
    height: 2.35rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface-hover);
    color: var(--text);
    text-decoration: none;
  }

  .back:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 1.15rem;
    max-width: 36rem;
  }

  .lead,
  .hint,
  .error {
    margin: 0;
    font-size: 0.85rem;
  }

  .lead,
  .hint {
    color: var(--text-muted);
  }

  .error {
    color: var(--danger);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  label {
    color: var(--text-muted);
    font-size: 0.78rem;
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  .stepper {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .stepper input {
    box-sizing: border-box;
    width: 4.2rem;
    padding: 0.45rem 0.4rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.9rem;
    text-align: center;
  }

  .stepper input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .stepper button {
    display: inline-flex;
    box-sizing: border-box;
    width: 2.1rem;
    height: 2.1rem;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface-hover);
    color: var(--text);
    font: inherit;
    font-size: 1rem;
    cursor: pointer;
  }

  .stepper button:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }

  .stepper button:disabled {
    color: var(--text-faint);
    cursor: not-allowed;
  }

  .unit {
    color: var(--text-faint);
    font-size: 0.78rem;
  }

  .preview {
    margin: 0;
    padding: 0.75rem 0.85rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    line-height: 1.4;
    white-space: pre;
  }
</style>
