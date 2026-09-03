<script lang="ts">
  import FileTreeContextMenu from "./FileTreeContextMenu.svelte";
  import FileTreeRow from "./FileTreeRow.svelte";
  import { flattenTree, type DraftKind, type TreeRow } from "$lib/file-tree";
  import { fileTree } from "$lib/file-tree.svelte";
  import type { TreeNode } from "$lib/workspace.svelte";

  let {
    nodes,
    selected,
    onSelect,
    onDelete,
    onCreate,
    onRename,
  }: {
    nodes: TreeNode[];
    selected: string | null;
    onSelect: (path: string) => void;
    onDelete: (path: string, kind: DraftKind) => void;
    onCreate: (name: string) => void;
    onRename: (name: string) => void;
  } = $props();

  let rows = $derived(
    flattenTree(nodes, { expanded: fileTree.expanded, draft: fileTree.draft }),
  );

  let menu = $state<{ x: number; y: number; path: string; kind: DraftKind } | null>(null);

  function rowKey(row: TreeRow): string {
    return row.kind === "draft" ? "draft" : row.path;
  }

  function closeMenu(): void {
    menu = null;
  }

  function openMenu(row: TreeRow, event: MouseEvent): void {
    if (row.kind !== "file" && row.kind !== "dir") return;
    menu = { x: event.clientX, y: event.clientY, path: row.path, kind: row.kind };
  }

  function startRename(path: string, kind: DraftKind): void {
    closeMenu();
    fileTree.startRename(path, kind);
  }

  function deleteTarget(path: string, kind: DraftKind): void {
    closeMenu();
    onDelete(path, kind);
  }

  function rowKind(row: TreeRow): DraftKind {
    return row.kind === "dir" ? "dir" : "file";
  }
</script>

<svelte:window
  onkeydown={(event) => {
    if (!menu) return;

    if (event.key === "F2") {
      event.preventDefault();
      startRename(menu.path, menu.kind);
      return;
    }

    if (event.key === "Delete") {
      event.preventDefault();
      deleteTarget(menu.path, menu.kind);
    }
  }}
/>

<div class="tree" role="tree" aria-label="Archivos de la carpeta">
  {#each rows as row (rowKey(row))}
    <FileTreeRow
      {row}
      selected={row.path === selected || row.path === menu?.path}
      renaming={fileTree.rename?.path === row.path}
      invalid={fileTree.draftError !== null}
      onActivate={() => {
        if (row.kind === "dir") fileTree.toggle(row.path);
        else if (row.kind === "file") onSelect(row.path);
      }}
      onExpand={() => fileTree.expand(row.path)}
      onCollapse={() => fileTree.toggle(row.path)}
      onDelete={() => deleteTarget(row.path, rowKind(row))}
      onRename={() => startRename(row.path, rowKind(row))}
      onContextMenu={(event) => openMenu(row, event)}
      onCommitDraft={onCreate}
      onCancelDraft={() => fileTree.cancelDraft()}
      onCommitRename={onRename}
      onCancelRename={() => fileTree.cancelRename()}
    />
  {/each}
</div>

{#if menu}
  {@const target = menu}
  <FileTreeContextMenu
    x={target.x}
    y={target.y}
    onDelete={() => deleteTarget(target.path, target.kind)}
    onRename={() => startRename(target.path, target.kind)}
    onClose={closeMenu}
  />
{/if}

<style>
  .tree {
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding-block: 0.15rem;
  }
</style>
