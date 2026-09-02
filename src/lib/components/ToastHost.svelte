<script lang="ts">
  import { fly } from "svelte/transition";
  import { toastsForPlacement } from "$lib/toast";
  import { toasts } from "$lib/toast.svelte";
</script>

<div class="host top" aria-live="polite">
  {#each toastsForPlacement(toasts.items, "top-right") as toast (toast.id)}
    <div class="toast" transition:fly={{ y: -12, duration: 180 }}>
      {toast.message}
    </div>
  {/each}
</div>

<div class="host bottom" aria-live="polite">
  {#each toastsForPlacement(toasts.items, "bottom-right") as toast (toast.id)}
    <div class="toast" transition:fly={{ y: 12, duration: 180 }}>
      {toast.message}
    </div>
  {/each}
</div>

<style>
  .host {
    position: fixed;
    right: 1.25rem;
    z-index: 80;
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    pointer-events: none;
  }

  .host.top {
    top: 1.25rem;
  }

  .host.bottom {
    bottom: 1.25rem;
  }

  .toast {
    padding: 0.7rem 0.95rem;
    border: 1px solid var(--border);
    border-left: 3px solid var(--accent);
    border-radius: 6px;
    background: var(--surface);
    color: var(--text);
    font-size: 0.9rem;
    box-shadow: 0 10px 28px var(--shadow);
  }
</style>
