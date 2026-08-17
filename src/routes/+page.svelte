<script lang="ts">
  import { onDestroy } from "svelte";
  import FileTree from "$lib/components/FileTree.svelte";
  import MarkdownEditor from "$lib/components/MarkdownEditor.svelte";
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
  <main class="welcome">
    <h1>idioteque</h1>
    <p>Abre una carpeta para ver y editar sus archivos markdown.</p>
    <button type="button" class="primary" onclick={() => workspace.openFolder()}>
      Abrir carpeta
    </button>
    {#if workspace.error}
      <p class="error">{workspace.error}</p>
    {/if}
  </main>
{:else}
  <div class="workspace">
    <aside>
      <header>
        <span class="root" title={workspace.root}>{workspace.root}</span>
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

      {#if workspace.error}
        <p class="error banner">{workspace.error}</p>
      {/if}
    </section>
  </div>
{/if}

<style>
  :global(:root) {
    --bg: #14161a;
    --surface: #191c21;
    --surface-hover: #22262d;
    --border: #2a2f37;
    --text: #e4e6ea;
    --text-muted: #9aa1ad;
    --text-faint: #666d79;
    --accent: #7aa2f7;
    --accent-soft: #7aa2f722;
    --danger: #f7768e;
    --font-ui: Inter, system-ui, -apple-system, sans-serif;
    --font-mono: "JetBrains Mono", "SF Mono", ui-monospace, monospace;

    color-scheme: dark;
  }

  :global(html),
  :global(body) {
    height: 100%;
    margin: 0;
  }

  :global(body) {
    background: var(--bg);
    color: var(--text);
    font-family: var(--font-ui);
    -webkit-font-smoothing: antialiased;
  }

  .welcome {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    height: 100vh;
    text-align: center;
  }

  .welcome h1 {
    margin: 0;
    font-size: 1.6rem;
    font-weight: 600;
  }

  .welcome p {
    max-width: 34rem;
    margin: 0;
    color: var(--text-muted);
  }

  .workspace {
    display: grid;
    grid-template-columns: 16rem 1fr;
    height: 100vh;
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

  code {
    font-family: var(--font-mono);
    font-size: 0.85em;
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
