<script lang="ts">
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import FilePlus from "@lucide/svelte/icons/file-plus";
  import FileText from "@lucide/svelte/icons/file-text";
  import Folder from "@lucide/svelte/icons/folder";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import FolderPlus from "@lucide/svelte/icons/folder-plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import type { TreeRow } from "$lib/file-tree";

  let {
    row,
    selected,
    invalid,
    onActivate,
    onExpand,
    onCollapse,
    onDelete,
    onCommitDraft,
    onCancelDraft,
  }: {
    row: TreeRow;
    selected: boolean;
    invalid: boolean;
    onActivate: () => void;
    onExpand: () => void;
    onCollapse: () => void;
    onDelete: () => void;
    onCommitDraft: (name: string) => void;
    onCancelDraft: () => void;
  } = $props();

  let draftName = $state("");
  let input = $state<HTMLInputElement | null>(null);

  $effect(() => {
    input?.focus();
  });

  function onRowKeydown(event: KeyboardEvent): void {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onActivate();
      return;
    }

    if (event.key === "Delete" && row.kind === "file") {
      event.preventDefault();
      event.stopPropagation();
      onDelete();
      return;
    }

    if (row.kind !== "dir") return;

    if (event.key === "ArrowRight" && !row.expanded) {
      event.preventDefault();
      onExpand();
    } else if (event.key === "ArrowLeft" && row.expanded) {
      event.preventDefault();
      onCollapse();
    }
  }

  function onDraftKeydown(event: KeyboardEvent): void {
    if (event.key === "Enter") {
      event.preventDefault();
      onCommitDraft(draftName);
      return;
    }

    if (event.key === "Escape") {
      event.preventDefault();
      onCancelDraft();
    }
  }
</script>

{#if row.kind === "draft"}
  <div class="row draft" style:--depth={row.depth} role="treeitem" aria-selected="true">
    <span class="twisty" aria-hidden="true"></span>
    <span class="icon" aria-hidden="true">
      {#if row.draftKind === "dir"}
        <FolderPlus size={14} strokeWidth={1.75} />
      {:else}
        <FilePlus size={14} strokeWidth={1.75} />
      {/if}
    </span>
    <!-- Focused on mount so the user types the name right away, like VS Code. -->
    <input
      bind:this={input}
      bind:value={draftName}
      class="field"
      class:invalid
      type="text"
      spellcheck="false"
      autocomplete="off"
      aria-invalid={invalid}
      aria-label={row.draftKind === "dir" ? "Nombre de la carpeta" : "Nombre del archivo"}
      placeholder={row.draftKind === "dir" ? "carpeta" : "nombre.md"}
      onkeydown={onDraftKeydown}
      onblur={onCancelDraft}
    />
  </div>
{:else}
  <div
    class="row"
    class:selected
    style:--depth={row.depth}
    role="treeitem"
    tabindex="0"
    aria-level={row.depth + 1}
    aria-selected={selected}
    aria-expanded={row.kind === "dir" ? row.expanded : undefined}
    title={row.path}
    onclick={onActivate}
    onkeydown={onRowKeydown}
  >
    <span class="twisty" aria-hidden="true">
      {#if row.kind === "dir"}
        {#if row.expanded}
          <ChevronDown size={14} strokeWidth={2} />
        {:else}
          <ChevronRight size={14} strokeWidth={2} />
        {/if}
      {/if}
    </span>
    <span class="icon" aria-hidden="true">
      {#if row.kind === "dir"}
        {#if row.expanded}
          <FolderOpen size={14} strokeWidth={1.75} />
        {:else}
          <Folder size={14} strokeWidth={1.75} />
        {/if}
      {:else}
        <FileText size={14} strokeWidth={1.75} />
      {/if}
    </span>
    <span class="name">{row.name}</span>
    {#if row.kind === "file"}
      <button
        type="button"
        class="delete"
        aria-label="Borrar {row.name}"
        onclick={(event) => {
          event.stopPropagation();
          onDelete();
        }}
      >
        <Trash2 size={13} strokeWidth={1.75} aria-hidden="true" />
      </button>
    {/if}
  </div>
{/if}

<style>
  .row {
    --indent-step: 0.72rem;

    display: flex;
    align-items: center;
    gap: 0.2rem;
    box-sizing: border-box;
    width: 100%;
    min-width: 0;
    height: 1.6rem;
    padding-inline: calc(0.25rem + var(--depth, 0) * var(--indent-step)) 0.25rem;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 0.8rem;
    line-height: 1.25;
    white-space: nowrap;
    cursor: pointer;
  }

  .row:hover {
    background: var(--surface-hover);
  }

  .row.selected {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .row:focus-visible {
    outline: 1px solid var(--accent);
    outline-offset: -1px;
  }

  .twisty,
  .icon {
    display: inline-flex;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    width: 0.95rem;
    color: var(--text-faint);
  }

  .row.selected .icon,
  .row.selected .twisty {
    color: var(--accent);
  }

  .name {
    flex: 1;
    align-self: stretch;
    min-width: 0;
    overflow: hidden;
    line-height: 1.6rem;
    text-overflow: ellipsis;
  }

  .delete {
    display: inline-flex;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    width: 1.25rem;
    height: 1.25rem;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--danger);
    opacity: 0;
    cursor: pointer;
  }

  .row:hover .delete,
  .row.selected .delete,
  .delete:focus-visible {
    opacity: 1;
  }

  .delete:hover {
    background: var(--surface);
  }

  .row.draft {
    cursor: default;
  }

  .field {
    flex: 1;
    min-width: 0;
    height: 1.2rem;
    padding: 0 0.25rem;
    border: 1px solid var(--accent);
    border-radius: 3px;
    background: var(--bg);
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 0.75rem;
  }

  .field:focus {
    outline: none;
  }

  .field.invalid {
    border-color: var(--danger);
  }
</style>
