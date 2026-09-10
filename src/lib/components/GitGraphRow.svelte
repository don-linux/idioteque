<script lang="ts">
  import { coincidenceHint, type DivergenceMark } from "$lib/git-divergence";
  import {
    LANE_HEIGHT,
    NODE_RADIUS,
    laneCenter,
    rowEdges,
    type LaneRow,
  } from "$lib/git-lanes";

  let {
    label,
    row,
    width,
    mark,
  }: {
    label: string;
    row: LaneRow;
    width: number;
    mark: DivergenceMark | undefined;
  } = $props();

  let edges = $derived(rowEdges(row));
  let cx = $derived(laneCenter(row.column));
  let cy = $derived(LANE_HEIGHT / 2);
  let kind = $derived(mark?.kind ?? "current");
  let hint = $derived(kind === "base" ? coincidenceHint(mark?.bases ?? []) : "");
</script>

<li class="row" class:shared={kind === "shared" || kind === "base"} class:other={kind === "other"}>
  <svg
    class="gutter"
    width={width}
    height={LANE_HEIGHT}
    viewBox={`0 0 ${width} ${LANE_HEIGHT}`}
    aria-hidden="true"
  >
    {#each edges as edge, index (index)}
      <path class="lane lane-{edge.color}" d={edge.d} />
    {/each}
    {#if row.merge}
      <circle class="ring lane-{row.color}" cx={cx} cy={cy} r={NODE_RADIUS + 2} />
    {/if}
    <circle class="node node-{row.color}" class:head={row.head} cx={cx} cy={cy} r={NODE_RADIUS} />
  </svg>
  <div class="meta">
    <span class="label">{label}</span>
    {#if hint}
      <span class="hint">{hint}</span>
    {/if}
  </div>
</li>

<style>
  .row {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 0.35rem;
    padding-right: 0.5rem;
    color: var(--text);
  }

  .row.other {
    color: var(--text-muted);
  }

  .row.shared .label {
    font-weight: 600;
  }

  .gutter {
    flex-shrink: 0;
    display: block;
  }

  .lane {
    fill: none;
    stroke-width: 1.6;
  }

  .node,
  .ring {
    stroke-width: 1.5;
  }

  .ring {
    fill: none;
  }

  .lane-0,
  .node-0 {
    stroke: var(--accent);
  }

  .node-0 {
    fill: var(--accent);
  }

  .lane-1,
  .node-1 {
    stroke: var(--syntax-function, #7aa2f7);
  }

  .node-1 {
    fill: var(--syntax-function, #7aa2f7);
  }

  .lane-2,
  .node-2 {
    stroke: var(--syntax-string, #9ece6a);
  }

  .node-2 {
    fill: var(--syntax-string, #9ece6a);
  }

  .lane-3,
  .node-3 {
    stroke: var(--syntax-keyword, #bb9af7);
  }

  .node-3 {
    fill: var(--syntax-keyword, #bb9af7);
  }

  .lane-4,
  .node-4 {
    stroke: var(--syntax-number, #ff9e64);
  }

  .node-4 {
    fill: var(--syntax-number, #ff9e64);
  }

  .lane-5,
  .node-5 {
    stroke: var(--syntax-type, #2ac3de);
  }

  .node-5 {
    fill: var(--syntax-type, #2ac3de);
  }

  .node.head {
    fill: var(--surface);
  }

  .meta {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
    justify-content: center;
    padding: 0.1rem 0;
  }

  .label {
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    line-height: 1.2;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hint {
    color: var(--accent);
    font-size: 0.62rem;
    line-height: 1.2;
  }
</style>
