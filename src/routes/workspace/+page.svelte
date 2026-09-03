<script lang="ts">
  import EditorPane from "$lib/components/EditorPane.svelte";
  import FileTreePanel from "$lib/components/FileTreePanel.svelte";
  import PanelSplitter from "$lib/components/PanelSplitter.svelte";
  import TerminalHost from "$lib/components/TerminalHost.svelte";
  import { appConfig } from "$lib/app-config.svelte";
  import { terminal } from "$lib/terminal.svelte";
  import { panels } from "$lib/workspace-panels.svelte";
  import { workspace } from "$lib/workspace.svelte";

  let terminals = $derived(terminal.surface === "terminals");
  let peeking = $derived(terminal.peeking);
  let showTree = $derived(panels.treeVisible);

  $effect(() => {
    if (appConfig.loaded) panels.hydrate(appConfig.layout);
  });
</script>

<svelte:window onresize={() => panels.fit(window.innerWidth, window.innerHeight)} />

{#if workspace.root !== null}
  <div
    class="workspace"
    class:with-tree={showTree && !terminals}
    class:term-bottom={peeking && terminal.dock === "bottom"}
    class:term-right={peeking && terminal.dock === "right"}
    class:surface-terminals={terminals}
    style:--tree-width="{panels.treeWidth}px"
    style:--term-size="{terminal.size}px"
    style:--park-width="{terminal.parkWidth}px"
    style:--park-height="{terminal.parkHeight}px"
  >
    {#if showTree}
      <FileTreePanel parked={terminals} />
      {#if !terminals}
        <div class="sash">
          <PanelSplitter
            axis="x"
            grow="forward"
            size={panels.treeWidth}
            label="Redimensionar el árbol de archivos"
            onSize={(pixels) => panels.setTreeWidth(pixels, window.innerWidth)}
            onCommit={() => panels.commitResize()}
          />
        </div>
      {/if}
    {/if}

    <EditorPane parked={terminals} />

    {#if terminal.started}
      <div class="term-slot" class:parked={!peeking && !terminals}>
        {#if peeking}
          <PanelSplitter
            axis={terminal.dock === "bottom" ? "y" : "x"}
            grow="backward"
            size={terminal.size}
            label="Redimensionar la terminal"
            onSize={(pixels) =>
              terminal.setSize(
                pixels,
                terminal.dock === "bottom" ? window.innerHeight : window.innerWidth,
                panels.treeSpace,
              )}
            onCommit={() => panels.commitResize()}
          />
        {/if}
        <TerminalHost cwd={workspace.root} />
      </div>
    {/if}
  </div>
{/if}

<style>
  /*
   * Four regions, two of them optional. The tracks are named so a hidden tree or
   * a parked terminal simply drops out of the template, and every visible region
   * keeps a definite size: xterm measures its own box and needs one.
   */
  .workspace {
    display: grid;
    flex: 1;
    grid-template-columns: 1fr;
    grid-template-rows: 1fr;
    grid-template-areas: "editor";
    width: 100%;
    height: 100%;
    min-height: 0;
  }

  .workspace.with-tree {
    grid-template-columns: var(--tree-width) 4px 1fr;
    grid-template-areas: "tree sash editor";
  }

  .workspace.term-bottom {
    grid-template-rows: 1fr var(--term-size);
    grid-template-areas: "editor" "term";
  }

  .workspace.with-tree.term-bottom {
    grid-template-columns: var(--tree-width) 4px 1fr;
    grid-template-rows: 1fr var(--term-size);
    grid-template-areas: "tree sash editor" "tree sash term";
  }

  .workspace.term-right {
    grid-template-columns: 1fr var(--term-size);
    grid-template-areas: "editor term";
  }

  .workspace.with-tree.term-right {
    grid-template-columns: var(--tree-width) 4px 1fr var(--term-size);
    grid-template-areas: "tree sash editor term";
  }

  .workspace.surface-terminals {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    left: 0;
    display: block;
    width: auto;
    height: auto;
  }

  .sash {
    display: flex;
    grid-area: sash;
    min-width: 0;
  }

  .term-slot {
    display: flex;
    box-sizing: border-box;
    grid-area: term;
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
    border-top: 1px solid var(--border);
  }

  .workspace.term-right .term-slot {
    flex-direction: row;
    border-left: 1px solid var(--border);
  }

  .workspace.surface-terminals .term-slot {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    left: 0;
    display: block;
    width: auto;
    height: auto;
    min-height: 0;
    border: 0;
  }

  /* Alive but out of sight: xterm keeps a measurable box so cols/rows stay valid. */
  .term-slot.parked {
    position: fixed;
    top: 0;
    left: -12000px;
    width: var(--park-width);
    height: var(--park-height);
    overflow: hidden;
    pointer-events: none;
    z-index: -1;
  }
</style>
