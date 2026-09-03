<script lang="ts">
  import FileTreeRow from "./FileTreeRow.svelte";
  import { flattenTree, type TreeRow } from "$lib/file-tree";
  import { fileTree } from "$lib/file-tree.svelte";
  import type { TreeNode } from "$lib/workspace.svelte";

  let {
    nodes,
    selected,
    onSelect,
    onDelete,
    onCreate,
  }: {
    nodes: TreeNode[];
    selected: string | null;
    onSelect: (path: string) => void;
    onDelete: (path: string) => void;
    onCreate: (name: string) => void;
  } = $props();

  let rows = $derived(
    flattenTree(nodes, { expanded: fileTree.expanded, draft: fileTree.draft }),
  );

  function rowKey(row: TreeRow): string {
    return row.kind === "draft" ? "draft" : row.path;
  }
</script>

<div class="tree" role="tree" aria-label="Archivos de la carpeta">
  {#each rows as row (rowKey(row))}
    <FileTreeRow
      {row}
      selected={row.kind === "file" && row.path === selected}
      invalid={fileTree.draftError !== null}
      onActivate={() => {
        if (row.kind === "dir") fileTree.toggle(row.path);
        else if (row.kind === "file") onSelect(row.path);
      }}
      onExpand={() => fileTree.expand(row.path)}
      onCollapse={() => fileTree.toggle(row.path)}
      onDelete={() => onDelete(row.path)}
      onCommitDraft={onCreate}
      onCancelDraft={() => fileTree.cancelDraft()}
    />
  {/each}
</div>

<style>
  .tree {
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding-block: 0.15rem;
  }
</style>
