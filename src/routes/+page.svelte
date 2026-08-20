<script lang="ts">
  import { onDestroy } from "svelte";
  import Settings from "@lucide/svelte/icons/settings";
  import FileTree from "$lib/components/FileTree.svelte";
  import MarkdownEditor from "$lib/components/MarkdownEditor.svelte";
  import RecentGrid from "$lib/components/RecentGrid.svelte";
  import TerminalPanel from "$lib/components/TerminalPanel.svelte";
  import { appConfig } from "$lib/app-config.svelte";
  import { terminal } from "$lib/terminal.svelte";
  import { workspace } from "$lib/workspace.svelte";

  let status = $derived.by(() => {
    if (workspace.saveState === "error") return "error al guardar";
    if (workspace.saveState === "saving") return "guardando";
    if (workspace.dirty) return "sin guardar";
    if (workspace.saveState === "saved") return "guardado";
    return "";
  });

  let stopResize: (() => void) | null = null;

  function startResize(event: PointerEvent): void {
    event.preventDefault();
    stopResize?.();

    const vertical = terminal.dock === "bottom";
    const start = vertical ? event.clientY : event.clientX;
    const startSize = terminal.size;
    const viewport = vertical ? window.innerHeight : window.innerWidth;

    function move(next: PointerEvent): void {
      const current = vertical ? next.clientY : next.clientX;
      terminal.setSize(startSize + (start - current), viewport);
    }

    function up(): void {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      stopResize = null;
    }

    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    stopResize = up;
  }

  onDestroy(() => {
    stopResize?.();
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
      <div class="header-actions">
        <a href="/configuracion" class="icon" aria-label="Configuración" title="Configuración">
          <Settings size={18} strokeWidth={1.75} aria-hidden="true" />
        </a>
        <button type="button" class="primary" onclick={() => workspace.openFolder()}>
          Abrir carpeta
        </button>
      </div>
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
  <div
    class="workspace"
    class:term-bottom={terminal.open && terminal.dock === "bottom"}
    class:term-right={terminal.open && terminal.dock === "right"}
    style:--term-size="{terminal.size}px"
    style:--park-width="{terminal.parkWidth}px"
    style:--park-height="{terminal.parkHeight}px"
  >
    <aside>
      <header>
        <span class="root" title={workspace.root}>{workspace.root}</span>
        <a href="/configuracion" class="icon compact" aria-label="Configuración" title="Configuración">
          <Settings size={16} strokeWidth={1.75} aria-hidden="true" />
        </a>
        <button type="button" onclick={() => workspace.closeWorkspace()}>Inicio</button>
        <button type="button" onclick={() => workspace.openFolder()}>Cambiar</button>
      </header>

      {#if workspace.hasMarkdown}
        <nav>
          <FileTree
            nodes={workspace.tree}
            selected={workspace.currentPath}
            onSelect={(path) => workspace.openFile(path)}
            onDelete={(path) => workspace.deleteFile(path)}
          />
        </nav>
      {:else}
        <p class="hint">Esta carpeta no tiene archivos markdown.</p>
      {/if}
    </aside>

    <section class="editor-col">
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

      {#if workspace.error || appConfig.error || terminal.error}
        <p class="error banner">{workspace.error ?? appConfig.error ?? terminal.error}</p>
      {/if}
    </section>

    {#if terminal.started}
      <div
        class={["term-slot", { parked: !terminal.open }]}
      >
        {#if terminal.open}
          <button
            type="button"
            class="split"
            aria-label="Redimensionar terminal"
            onpointerdown={startResize}
          ></button>
        {/if}
        <TerminalPanel cwd={workspace.root} />
      </div>
    {/if}
  </div>
{/if}

<style>
  .home {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    height: 100%;
    overflow: auto;
    padding: 2rem 2rem 3rem;
  }

  .home header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1.5rem;
  }

  .header-actions {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    gap: 0.5rem;
  }

  a.icon {
    display: inline-flex;
    box-sizing: border-box;
    align-items: center;
    justify-content: center;
    width: 2.35rem;
    height: 2.35rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface-hover);
    color: var(--text);
    text-decoration: none;
  }

  a.icon:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  a.icon.compact {
    flex-shrink: 0;
    width: 1.85rem;
    height: 1.85rem;
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
    grid-template-rows: 1fr;
    height: 100%;
    min-height: 0;
  }

  .workspace.term-bottom {
    grid-template-columns: 16rem 1fr;
    grid-template-rows: 1fr var(--term-size);
  }

  .workspace.term-right {
    grid-template-columns: 16rem 1fr var(--term-size);
    grid-template-rows: 1fr;
  }

  aside {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    border-right: 1px solid var(--border);
    background: var(--surface);
  }

  .workspace.term-bottom aside {
    grid-row: 1 / -1;
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

  .editor-col {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .workspace.term-bottom .editor-col {
    grid-column: 2;
    grid-row: 1;
  }

  .workspace.term-right .editor-col {
    grid-column: 2;
  }

  .editor-col header {
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

  .term-slot {
    display: flex;
    box-sizing: border-box;
    align-self: stretch;
    width: 100%;
    min-width: 0;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    background: var(--bg);
  }

  .workspace.term-bottom .term-slot {
    flex-direction: column;
    grid-column: 2;
    grid-row: 2;
    border-top: 1px solid var(--border);
  }

  .workspace.term-right .term-slot {
    flex-direction: row;
    grid-column: 3;
    border-left: 1px solid var(--border);
  }

  .term-slot.parked {
    position: fixed;
    top: 0;
    left: -12000px;
    grid-column: 1;
    grid-row: 1;
    width: var(--park-width);
    height: var(--park-height);
    overflow: hidden;
    visibility: hidden;
    pointer-events: none;
    z-index: -1;
  }

  .split {
    box-sizing: border-box;
    flex-shrink: 0;
    padding: 0;
    border: 0;
    border-radius: 0;
    background: var(--border);
  }

  .workspace.term-bottom .split {
    width: 100%;
    height: 4px;
    cursor: row-resize;
  }

  .workspace.term-right .split {
    width: 4px;
    height: 100%;
    cursor: col-resize;
  }

  .split:hover {
    background: var(--accent);
    border-color: var(--accent);
    color: inherit;
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
