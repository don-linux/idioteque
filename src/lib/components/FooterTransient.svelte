<script lang="ts">
  import Save from "@lucide/svelte/icons/save";
  import { workspace } from "$lib/workspace.svelte";
</script>

<div class="transient">
  {#if workspace.dirty}
    <button
      type="button"
      class="action"
      aria-label="Guardar (Ctrl+S)"
      title="Guardar (Ctrl+S)"
      onclick={() => void workspace.save()}
    >
      <Save size={16} strokeWidth={1.75} aria-hidden="true" />
    </button>
  {/if}
</div>

<style>
  .transient {
    display: flex;
    flex: 1;
    min-width: 0;
    align-items: center;
    gap: 0.15rem;
  }

  .action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.65rem;
    height: 1.65rem;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .action:hover {
    background: var(--surface-hover);
    color: var(--text);
  }
</style>
