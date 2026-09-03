<script lang="ts">
  import FilePlus from "@lucide/svelte/icons/file-plus";
  import FolderPlus from "@lucide/svelte/icons/folder-plus";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import FileTree from "./FileTree.svelte";
  import { draftParentFor, type DraftKind } from "$lib/file-tree";
  import { fileTree } from "$lib/file-tree.svelte";
  import { workspace } from "$lib/workspace.svelte";

  let { parked = false }: { parked?: boolean } = $props();

  let refreshing = $state(false);

  function startDraft(kind: DraftKind): void {
    const parent = draftParentFor(workspace.currentPath, (path) => workspace.isDirectory(path));
    fileTree.startDraft(kind, parent);
  }

  async function refresh(): Promise<void> {
    refreshing = true;
    try {
      await workspace.refreshTree();
    } finally {
      refreshing = false;
    }
  }
</script>

<aside
  class:parked
  oncontextmenu={(event) => {
    event.preventDefault();
  }}
>
  <header>
    <span class="folder" title={workspace.root}>{workspace.folderName}</span>
  </header>

  <div class="toolbar">
    <button
      type="button"
      class="tool"
      aria-label="Crear archivo"
      title="Crear archivo"
      onclick={() => startDraft("file")}
    >
      <FilePlus size={15} strokeWidth={1.75} aria-hidden="true" />
    </button>
    <button
      type="button"
      class="tool"
      aria-label="Crear carpeta"
      title="Crear carpeta"
      onclick={() => startDraft("dir")}
    >
      <FolderPlus size={15} strokeWidth={1.75} aria-hidden="true" />
    </button>
    <button
      type="button"
      class="tool"
      class:spinning={refreshing}
      aria-label="Refrescar"
      title="Refrescar"
      onclick={() => void refresh()}
    >
      <RefreshCw size={15} strokeWidth={1.75} aria-hidden="true" />
    </button>
  </div>

  <div class="body">
    {#if workspace.hasEntries || fileTree.draft}
      <FileTree
        nodes={workspace.tree}
        selected={workspace.currentPath}
        onSelect={(path) => workspace.openFile(path)}
        onDelete={(path, kind) => {
          if (kind === "dir") void workspace.deleteFolder(path);
          else void workspace.deleteFile(path);
        }}
        onCreate={(name) => {
          const draft = fileTree.draft;
          if (draft) void workspace.createEntry(draft.kind, draft.parent, name);
        }}
        onRename={(name) => {
          const rename = fileTree.rename;
          if (rename) void workspace.renameEntry(rename.path, rename.kind, name);
        }}
      />
    {:else}
      <p class="hint">Esta carpeta está vacía.</p>
    {/if}
  </div>

  {#if fileTree.draftError}
    <p class="draft-error">{fileTree.draftError}</p>
  {/if}
</aside>

<style>
  aside {
    display: flex;
    flex-direction: column;
    grid-area: tree;
    min-width: 0;
    min-height: 0;
    border-right: 1px solid var(--border);
    background: var(--surface);
  }

  aside.parked {
    position: fixed;
    top: 0;
    left: -12000px;
    width: var(--tree-width, 16rem);
    height: 80vh;
    overflow: hidden;
    pointer-events: none;
    z-index: -1;
  }

  header {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    height: 2rem;
    padding: 0 0.6rem;
  }

  .folder {
    min-width: 0;
    overflow: hidden;
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 0.75rem;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .toolbar {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    gap: 0.1rem;
    padding: 0 0.35rem 0.35rem;
    border-bottom: 1px solid var(--border);
  }

  .tool {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.5rem;
    height: 1.5rem;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .tool:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .tool.spinning {
    color: var(--accent);
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
  }

  .hint {
    margin: 0;
    padding: 0.5rem 0.6rem;
    color: var(--text-faint);
    font-size: 0.78rem;
  }

  .draft-error {
    flex-shrink: 0;
    margin: 0;
    padding: 0.35rem 0.6rem;
    border-top: 1px solid var(--border);
    color: var(--danger);
    font-size: 0.72rem;
  }
</style>
