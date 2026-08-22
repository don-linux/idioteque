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

  onMount(() => {
    const observer = new ResizeObserver((entries) => {
      const box = entries[0]?.contentRect;
      if (!box) return;
      if (box.width >= 2) hostWidth = box.width;
      if (box.height >= 2) hostHeight = box.height;
    });

    if (host) observer.observe(host);
    return () => observer.disconnect();
  });
</script>

<div
  class={["host", { tiles, peek: !tiles }]}
  style:--tile-cols={plan.cols}
  style:--tile-rows={plan.rows}
  bind:this={host}
>
  {#if tiles && terminal.sessions.length === 0}
    <p class="empty">Añade una terminal.</p>
  {:else}
    {#each terminal.sessions as session (session.id)}
      <div
        class={["cell", { hidden: !tiles && session.id !== terminal.activeId }]}
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
    width: 100%;
    min-width: 0;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    background: var(--bg);
  }

  .host.tiles {
    flex-wrap: wrap;
    align-content: stretch;
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
    flex-grow: 1;
    flex-shrink: 1;
    flex-basis: calc((100% - (var(--tile-cols) - 1) * 1px) / var(--tile-cols));
    width: calc((100% - (var(--tile-cols) - 1) * 1px) / var(--tile-cols));
    height: calc((100% - (var(--tile-rows) - 1) * 1px) / var(--tile-rows));
  }

  .host.peek .cell.hidden {
    position: fixed;
    top: 0;
    left: -12000px;
    flex: none;
    width: var(--park-width);
    height: var(--park-height);
    overflow: hidden;
    visibility: hidden;
    pointer-events: none;
    z-index: -1;
  }
</style>
