<script lang="ts">
  import TerminalPane from "$lib/components/TerminalPane.svelte";
  import { terminal } from "$lib/terminal.svelte";
  import { tilePlan } from "$lib/terminal-tile";

  let { cwd }: { cwd: string } = $props();

  let hostWidth = $state(0);
  let hostHeight = $state(0);

  let tiles = $derived(terminal.surface === "terminals");
  let plan = $derived(
    tilePlan(
      terminal.sessions.map((session) => session.id),
      hostWidth,
      hostHeight,
    ),
  );
</script>

<div
  class={["host", { tiles, peek: !tiles }]}
  style:--tile-cols={plan.cols}
  style:--tile-rows={plan.rows}
  style:--tile-units={plan.units}
  bind:clientWidth={hostWidth}
  bind:clientHeight={hostHeight}
>
  {#if tiles && terminal.sessions.length === 0}
    <p class="empty">Añade una terminal.</p>
  {:else}
    {#each terminal.sessions as session, index (session.id)}
      {@const cell = plan.cells[index]}
      <div
        class={["cell", { hidden: !tiles && session.id !== terminal.activeId }]}
        style:grid-column={cell?.column}
        style:grid-row={cell?.row}
      >
        <TerminalPane
          sessionId={session.id}
          {cwd}
          visible={terminal.isVisible(session.id)}
        />
      </div>
    {/each}
  {/if}
</div>

<style>
  .host {
    box-sizing: border-box;
    width: 100%;
    min-width: 0;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    background: var(--bg);
  }

  .host.peek {
    display: flex;
  }

  .host.tiles {
    position: absolute;
    inset: 0;
    display: grid;
    grid-template-columns: repeat(var(--tile-units), minmax(0, 1fr));
    grid-template-rows: repeat(var(--tile-rows), minmax(0, 1fr));
    gap: 1px;
    background: var(--border);
  }

  .empty {
    display: grid;
    place-content: center;
    width: 100%;
    height: 100%;
    margin: 0;
    color: var(--text-faint);
    font-size: 0.82rem;
  }

  .cell {
    position: relative;
    box-sizing: border-box;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    background: var(--bg);
  }

  .host.peek .cell {
    flex: 1;
    width: 100%;
    height: 100%;
  }

  .host.tiles .cell {
    width: 100%;
    height: 100%;
  }

  .host.peek .cell.hidden {
    position: fixed;
    top: 0;
    left: -12000px;
    flex: none;
    width: var(--park-width);
    height: var(--park-height);
    overflow: hidden;
    pointer-events: none;
    z-index: -1;
  }
</style>
