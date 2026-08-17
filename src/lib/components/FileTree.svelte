<script lang="ts">
  import type { TreeNode } from "$lib/workspace.svelte";
  import FileTree from "./FileTree.svelte";

  let {
    nodes,
    selected,
    onSelect,
  }: {
    nodes: TreeNode[];
    selected: string | null;
    onSelect: (path: string) => void;
  } = $props();
</script>

<ul>
  {#each nodes as node (node.path)}
    <li>
      {#if node.kind === "dir"}
        <span class="dir">{node.name}</span>
        <div class="children">
          {#if node.children.length > 0}
            <FileTree nodes={node.children} {selected} {onSelect} />
          {:else}
            <p class="empty">sin archivos .md</p>
          {/if}
        </div>
      {:else}
        <button
          type="button"
          class="file"
          class:selected={node.path === selected}
          onclick={() => onSelect(node.path)}
        >
          {node.name}
        </button>
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

  .file {
    display: block;
    width: 100%;
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

  .file:hover {
    background: var(--surface-hover);
  }

  .file.selected {
    background: var(--accent-soft);
    color: var(--accent);
  }
</style>
