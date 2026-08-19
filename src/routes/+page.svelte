<script lang="ts">
  import { onDestroy } from "svelte";
  import FileTree from "$lib/components/FileTree.svelte";
  import MarkdownEditor from "$lib/components/MarkdownEditor.svelte";
  import RecentGrid from "$lib/components/RecentGrid.svelte";
  import { appConfig } from "$lib/app-config.svelte";
  import { workspace } from "$lib/workspace.svelte";

  let status = $derived.by(() => {
    if (workspace.saveState === "error") return "error al guardar";
    if (workspace.saveState === "saving") return "guardando";
    if (workspace.dirty) return "sin guardar";
    if (workspace.saveState === "saved") return "guardado";
    return "";
  });

  onDestroy(() => {
    void workspace.flushSave();
  });
</script>

{#if workspace.root === null}
  <main class="home">
    <header>
      <div>
        <h1>idioteque</h1>
        <p>Abre una carpeta para ver y editar sus archivos markdown.</p>
      </div>
      <button type="button" class="primary" onclick={() => workspace.openFolder()}>
        Abrir carpeta
      </button>
    </header>

    {#if appConfig.recents.length === 0 && appConfig.loaded}
      <p class="empty">Todavía no hay carpetas en el historial.</p>
    {/if}

    <RecentGrid
      recents={appConfig.recents}
      onOpen={(path) => workspace.openRoot(path)}
      onRemove={(path) => appConfig.remove(path)}
    />

    {#if workspace.error || appConfig.error}
      <p class="error">{workspace.error ?? appConfig.error}</p>
    {/if}
  </main>
{:else}
  <div class="workspace">
    <aside>
      <header>
        <span class="root" title={workspace.root}>{workspace.root}</span>
        <button type="button" onclick={() => workspace.closeWorkspace()}>Inicio</button>
        <button type="button" onclick={() => workspace.openFolder()}>Cambiar</button>
      </header>

      {#if workspace.hasMarkdown}
        <nav>
          <FileTree
            nodes={workspace.tree}
            selected={workspace.currentPath}
            onSelect={(path) => workspace.openFile(path)}
          />
        </nav>
      {:else}
        <p class="hint">Esta carpeta no tiene archivos markdown.</p>
      {/if}
    </aside>

    <section>
      {#if workspace.currentPath === null}
        <p class="hint centered">Selecciona un archivo.</p>
      {:else}
        <header>
          <span class="path">{workspace.currentPath}</span>
          <span class="status" class:error={workspace.saveState === "error"}>{status}</span>
        </header>
        <MarkdownEditor
          content={workspace.content}
          onChange={(contents) => workspace.edit(contents)}
        />
      {/if}

      {#if workspace.error || appConfig.error}
        <p class="error banner">{workspace.error ?? appConfig.error}</p>
      {/if}
    </section>
  </div>
{/if}

<style>
  .home {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    min-height: 100%;
    padding: 2rem 2rem 3rem;
  }

  .home header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1.5rem;
  }

  .home h1 {
    margin: 0 0 0.35rem;
    font-size: 1.6rem;
    font-weight: 600;
  }

  .home p {
    max-width: 34rem;
    margin: 0;
    color: var(--text-muted);
  }

  .empty {
    color: var(--text-faint);
    font-size: 0.85rem;
  }

  .workspace {
    display: grid;
    grid-template-columns: 16rem 1fr;
    height: 100%;
  }

  aside {
    display: flex;
    flex-direction: column;
    min-width: 0;
    border-right: 1px solid var(--border);
    background: var(--surface);
  }

  aside header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 0.75rem;
    border-bottom: 1px solid var(--border);
  }

  .root {
    flex: 1;
    overflow: hidden;
    color: var(--text-muted);
    font-size: 0.75rem;
    white-space: nowrap;
    text-overflow: ellipsis;
    direction: rtl;
    text-align: left;
  }

  nav {
    flex: 1;
    overflow: auto;
    padding: 0.5rem;
  }

  section {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  section header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.6rem 1.5rem;
    border-bottom: 1px solid var(--border);
  }

  .path {
    overflow: hidden;
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 0.78rem;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .status {
    flex-shrink: 0;
    color: var(--text-faint);
    font-size: 0.72rem;
  }

  .status.error {
    color: var(--danger);
  }

  .hint {
    margin: 0;
    padding: 1rem;
    color: var(--text-faint);
    font-size: 0.82rem;
  }

  .centered {
    display: grid;
    flex: 1;
    place-content: center;
  }

  .error {
    color: var(--danger);
    font-size: 0.8rem;
  }

  .banner {
    margin: 0;
    padding: 0.6rem 1.5rem;
    border-top: 1px solid var(--border);
  }

  button {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.35rem 0.7rem;
    background: var(--surface-hover);
    color: var(--text);
    font: inherit;
    font-size: 0.8rem;
    cursor: pointer;
  }

  button:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  button.primary {
    padding: 0.5rem 1.1rem;
    font-size: 0.9rem;
  }
</style>
