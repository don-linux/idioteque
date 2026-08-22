<script lang="ts">
  import { onMount } from "svelte";
  import TerminalPane from "$lib/components/TerminalPane.svelte";
  import { terminal } from "$lib/terminal.svelte";
  import { tilePlan } from "$lib/terminal-tile";

  let { cwd }: { cwd: string } = $props();

  let host: HTMLDivElement | undefined;
  let hostWidth = $state(1200);
  let hostHeight = $state(800);

  let tiles = $derived(terminal.surface === "terminals");
  let plan = $derived(tilePlan(
    terminal.sessions.map((session) => session.id),
    hostWidth,
    hostHeight,
  ));

  function measureHost(): void {
    if (!host) return;
    const box = host.getBoundingClientRect();
    if (box.width >= 2) hostWidth = box.width;
    if (box.height >= 2) hostHeight = box.height;
  }

  onMount(() => {
    const observer = new ResizeObserver(() => {
      measureHost();
    });

    if (host) observer.observe(host);
    return () => observer.disconnect();
  });

  $effect(() => {
    terminal.surface;
    const frame = requestAnimationFrame(() => {
      measureHost();
    });
    return () => {
      cancelAnimationFrame(frame);
    };
  });
</script>

<div
  class={["host", { tiles, peek: !tiles }]}
  style:--tile-cols={plan.cols}
  style:--tile-rows={plan.rows}
  style:--tile-units={plan.units}
  bind:this={host}
>
  {#if tiles && terminal.sessions.length === 0}
    <p class="empty">Añade una terminal.</p>
  {:else}
    {#each terminal.sessions as session, index (session.id)}
      <div
        class={["cell", { hidden: !tiles && session.id !== terminal.activeId }]}
        style:grid-column={plan.cells[index]?.column}
        style:grid-row={plan.cells[index]?.row}
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
    display: flex;
    flex: 1;
    align-self: stretch;
    width: 100%;
    min-width: 0;
    height: auto;
    min-height: 0;
    overflow: hidden;
    background: var(--bg);
  }

  .host.tiles {
    display: grid;
    grid-template-columns: repeat(var(--tile-units), minmax(0, 1fr));
    grid-template-rows: repeat(var(--tile-rows), minmax(0, 1fr));
    gap: 1px;
    background: var(--border);
  }

  .empty {
    display: grid;
    flex: 1;
    place-content: center;
    margin: 0;
    color: var(--text-faint);
    font-size: 0.82rem;
  }

  .cell {
    display: flex;
    box-sizing: border-box;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    background: var(--bg);
  }

  .host.peek .cell {
    flex: 1;
  }

  .host.tiles .cell {
    width: auto;
    height: auto;
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
