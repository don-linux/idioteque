<script lang="ts">
  import { onDestroy } from "svelte";
  import EditorTabs from "$lib/components/EditorTabs.svelte";
  import FileTree from "$lib/components/FileTree.svelte";
  import MarkdownEditor from "$lib/components/MarkdownEditor.svelte";
  import TerminalHost from "$lib/components/TerminalHost.svelte";
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
  let terminals = $derived(terminal.surface === "terminals");
  let peeking = $derived(terminal.peeking);

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
  });
</script>

{#if workspace.root !== null}
  <div
    class="workspace"
    class:term-bottom={peeking && terminal.dock === "bottom"}
    class:term-right={peeking && terminal.dock === "right"}
    class:surface-terminals={terminals}
    style:--term-size="{terminal.size}px"
    style:--park-width="{terminal.parkWidth}px"
    style:--park-height="{terminal.parkHeight}px"
  >
    <aside class:parked={terminals}>
      <header>
        <span class="root" title={workspace.root}>{workspace.root}</span>
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

    <section class="editor-col" class:parked={terminals}>
      {#if workspace.currentPath === null}
        <p class="hint centered">Selecciona un archivo.</p>
      {:else}
        <header>
          <EditorTabs />
          <span class="status" class:error={workspace.saveState === "error"}>{status}</span>
        </header>
        <MarkdownEditor
          path={workspace.currentPath}
          content={workspace.content}
          contentReady={workspace.contentReady}
          onChange={(contents) => workspace.edit(contents)}
        />
      {/if}

      {#if workspace.error || appConfig.error || terminal.error}
        <p class="error banner">{workspace.error ?? appConfig.error ?? terminal.error}</p>
      {/if}
    </section>

    {#if terminal.started}
      <div class={["term-slot", { parked: !peeking && !terminals, tiles: terminals }]}>
        {#if peeking}
          <button type="button" class="split" aria-label="Redimensionar terminal" onpointerdown={startResize}
          ></button>
        {/if}
        <TerminalHost cwd={workspace.root} />
      </div>
    {/if}
  </div>
{/if}

<style>
	.workspace {
		display: grid;
		flex: 1;
		grid-template-columns: 16rem 1fr;
		grid-template-rows: 1fr;
		width: 100%;
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

	.workspace.surface-terminals {
		position: absolute;
		inset: 0;
		display: block;
		width: 100%;
		height: 100%;
	}

  .workspace.surface-terminals aside,
  .workspace.surface-terminals .editor-col {
    display: none;
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
    align-items: stretch;
    gap: 1rem;
    min-height: 2.35rem;
    border-bottom: 1px solid var(--border);
  }

  .status {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    padding-right: 1.5rem;
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

	.workspace.surface-terminals .term-slot {
		position: absolute;
		inset: 0;
		display: block;
		width: 100%;
		height: 100%;
		min-height: 0;
		border: 0;
	}

  .term-slot.parked,
  aside.parked,
  .editor-col.parked {
    position: fixed;
    top: 0;
    left: -12000px;
    overflow: hidden;
    pointer-events: none;
    z-index: -1;
  }

  .term-slot.parked {
    grid-column: 1;
    grid-row: 1;
    width: var(--park-width);
    height: var(--park-height);
  }

  aside.parked {
    width: 16rem;
    height: 80vh;
  }

  .editor-col.parked {
    width: min(80vw, 1200px);
    height: 80vh;
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
</style>
