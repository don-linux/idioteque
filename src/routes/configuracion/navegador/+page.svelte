<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import type { CefRuntimeInfo } from "$lib/cef-runtime";
  import {
    COPY,
    DENYLIST_NONE,
    createNavegadorPage,
    runtimeLines,
  } from "./navegador-page";

  let info = $state.raw<CefRuntimeInfo | null>(null);
  let error = $state<string | null>(null);
  let status = $state<string | null>(null);
  let checking = $state(false);
  let lines = $derived(info ? runtimeLines(info) : null);

  const page = createNavegadorPage({
    invoke: (command) => invoke(command),
    onChange(next) {
      info = next.info;
      error = next.error;
      status = next.status;
      checking = next.checking;
    },
  });

  onMount(() => {
    void page.load();
    return () => page.dispose();
  });
</script>

<section class="section" aria-labelledby="browser-heading">
  <h2 id="browser-heading">{COPY.heading}</h2>
  <p class="lead">
    {COPY.lead}
  </p>

  {#if error}
    <p class="error">{error}</p>
  {:else if lines}
    <ul class="rows">
      <li>{lines.chromium}</li>
      <li>{lines.cef}</li>
      <li>{lines.base}</li>
      <li>{lines.lastCheck}</li>
      {#if lines.pending}
        <li>{lines.pending}</li>
      {/if}
      <li>
        {lines.denylistLabel}
        {#if lines.denylist.length === 0}
          {DENYLIST_NONE}
        {:else}
          <ul class="denied">
            {#each lines.denylist as entry (entry)}
              <li>{entry}</li>
            {/each}
          </ul>
        {/if}
      </li>
    </ul>
  {:else}
    <p class="hint">{COPY.loading}</p>
  {/if}

  <div class="field">
    <button type="button" class="check" disabled={checking} onclick={() => void page.checkUpdates()}>
      <RefreshCw size={16} strokeWidth={1.75} aria-hidden="true" />
      {COPY.check}
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
