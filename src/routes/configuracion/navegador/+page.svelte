<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import {
    CEF_CHECKING,
    CEF_UNAVAILABLE,
    formatCheckedAt,
    formatDenyEntry,
    formatPendingPromotion,
    sourceLabel,
    type CefRuntimeInfo,
  } from "$lib/cef-runtime";

  let info = $state.raw<CefRuntimeInfo | null>(null);
  let error = $state<string | null>(null);
  let status = $state<string | null>(null);
  let checking = $state(false);
  let disposed = false;
  let disableTimer: ReturnType<typeof setTimeout> | undefined;
  let reloadTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    disposed = false;
    void load();
    return () => {
      disposed = true;
      if (disableTimer !== undefined) clearTimeout(disableTimer);
      if (reloadTimer !== undefined) clearTimeout(reloadTimer);
    };
  });

  async function load(): Promise<void> {
    try {
      const next = await invoke<CefRuntimeInfo>("cef_runtime_info");
      if (disposed) return;
      info = next;
      error = null;
    } catch {
      if (disposed) return;
      info = null;
      error = CEF_UNAVAILABLE;
    }
  }

  async function checkUpdates(): Promise<void> {
    if (checking) return;
    checking = true;
    status = CEF_CHECKING;

    try {
      await invoke("cef_check_updates");
    } catch (caught) {
      if (!disposed) status = messageFrom(caught);
    }

    disableTimer = setTimeout(() => {
      checking = false;
    }, 3000);

    reloadTimer = setTimeout(() => {
      void load().then(() => {
        if (!disposed && status === CEF_CHECKING) status = null;
      });
    }, 5000);
  }

  function messageFrom(caught: unknown): string {
    if (typeof caught === "string" && caught.trim()) return caught;
    if (caught instanceof Error && caught.message.trim()) return caught.message;
    return CEF_UNAVAILABLE;
  }
</script>

<section class="section" aria-labelledby="browser-heading">
  <h2 id="browser-heading">Navegador</h2>
  <p class="lead">
    Motor Chromium embebido. No hay nada que guardar aquí: es información y un botón.
  </p>

  {#if error}
    <p class="error">{error}</p>
  {:else if info}
    <ul class="rows">
      <li>
        Chromium actual: {info.current.chromiumVersion} ({sourceLabel(info.current.source)})
      </li>
      <li>CEF: {info.current.cefVersion}</li>
      <li>Base de fábrica: Chromium {info.base.chromiumVersion}</li>
      <li>Última comprobación: {formatCheckedAt(info.lastCheckAt)}</li>
      {#if info.pendingPromotion}
        <li>{formatPendingPromotion(info.pendingPromotion)}</li>
      {/if}
      <li>
        Versiones descartadas:
        {#if info.denylist.length === 0}
          ninguna
        {:else}
          <ul class="denied">
            {#each info.denylist as entry (`${entry.chromiumVersion}-${entry.at}`)}
              <li>{formatDenyEntry(entry)}</li>
            {/each}
          </ul>
        {/if}
      </li>
    </ul>
  {:else}
    <p class="hint">Cargando…</p>
  {/if}

  <div class="field">
    <button type="button" class="check" disabled={checking} onclick={() => void checkUpdates()}>
      <RefreshCw size={16} strokeWidth={1.75} aria-hidden="true" />
      Buscar actualización
    </button>
    {#if status}
      <p class="hint">{status}</p>
    {/if}
  </div>
</section>

<style>
  .section {
    display: flex;
    flex-direction: column;
    gap: 1.15rem;
    max-width: 36rem;
    padding: 2rem 2rem 3rem;
  }

  h2 {
    margin: 0 0 0.35rem;
    font-size: 1.05rem;
    font-weight: 600;
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

  .rows {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    margin: 0;
    padding: 0;
    list-style: none;
    color: var(--text);
    font-size: 0.9rem;
    line-height: 1.45;
  }

  .denied {
    margin: 0.4rem 0 0;
    padding-left: 1.15rem;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .check {
    display: inline-flex;
    box-sizing: border-box;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    width: fit-content;
    height: 2.5rem;
    padding: 0.55rem 1rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface-hover);
    color: var(--text);
    font: inherit;
    font-size: 0.95rem;
    font-weight: 600;
    cursor: pointer;
  }

  .check:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }

  .check:disabled {
    color: var(--text-faint);
    cursor: not-allowed;
  }
</style>
