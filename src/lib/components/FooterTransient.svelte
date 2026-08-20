<script lang="ts">
  import Save from "@lucide/svelte/icons/save";
  import Undo2 from "@lucide/svelte/icons/undo-2";
  import { editorSession } from "$lib/editor-session.svelte";
  import { workspace } from "$lib/workspace.svelte";
</script>

<div class="transient">
  {#if editorSession.canUndo}
    <button
      type="button"
      class="action"
      aria-label="Deshacer (Ctrl+Z)"
      title="Deshacer (Ctrl+Z)"
      onclick={() => editorSession.undo()}
    >
      <Undo2 size={16} strokeWidth={1.75} aria-hidden="true" />
    </button>
  {/if}
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
