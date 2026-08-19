<script lang="ts">
  import type { TreeNode } from "$lib/workspace.svelte";
  import FileTree from "./FileTree.svelte";

  let {
    nodes,
    selected,
    onSelect,
    onDelete,
  }: {
    nodes: TreeNode[];
    selected: string | null;
    onSelect: (path: string) => void;
    onDelete: (path: string) => void;
  } = $props();

  function onFileKeydown(event: KeyboardEvent, path: string): void {
    if (event.key !== "Delete") return;
    event.preventDefault();
    event.stopPropagation();
    onDelete(path);
  }
</script>

<ul>
  {#each nodes as node (node.path)}
    <li>
      {#if node.kind === "dir"}
        <span class="dir">{node.name}</span>
        <div class="children">
          {#if node.children.length > 0}
            <FileTree nodes={node.children} {selected} {onSelect} {onDelete} />
          {:else}
            <p class="empty">sin archivos .md</p>
          {/if}
        </div>
      {:else}
        <div class="row" class:selected={node.path === selected}>
          <button
            type="button"
            class="file"
            onclick={() => onSelect(node.path)}
            onkeydown={(event) => onFileKeydown(event, node.path)}
          >
            {node.name}
          </button>
          <button
            type="button"
            class="delete"
            aria-label="Borrar {node.name}"
            onclick={() => onDelete(node.path)}
          >
            ×
          </button>
        </div>
      {/if}
    </li>
  {/each}
</ul>

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .children {
    margin-left: 0.4rem;
    padding-left: 0.45rem;
    border-left: 1px solid var(--border);
  }

  .dir {
    display: block;
    padding: 0.2rem 0.4rem;
    color: var(--text-muted);
    font-size: 0.78rem;
    letter-spacing: 0.03em;
    text-transform: lowercase;
  }

  .empty {
    margin: 0;
    padding: 0.2rem 0.4rem;
    color: var(--text-faint);
    font-size: 0.72rem;
    font-style: italic;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 0.15rem;
    border-radius: 4px;
  }

  .row:hover,
  .row.selected {
    background: var(--surface-hover);
  }

  .row.selected {
    background: var(--accent-soft);
  }

  .file {
    flex: 1;
    min-width: 0;
    padding: 0.25rem 0.4rem;
    border: 0;
    border-radius: 4px;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 0.85rem;
    text-align: left;
    cursor: pointer;
  }

  .row.selected .file {
    color: var(--accent);
  }

  .delete {
    flex-shrink: 0;
    width: 1.4rem;
    height: 1.4rem;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-faint);
    font-size: 1rem;
    line-height: 1;
    opacity: 0;
    cursor: pointer;
  }

  .row:hover .delete,
  .row.selected .delete,
  .delete:focus-visible {
    opacity: 1;
  }

  .delete:hover {
    background: var(--surface-hover);
    color: var(--danger);
  }
</style>
