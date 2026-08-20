<script lang="ts">
  import {
    DEFAULT_FONT_LABEL,
    filterFonts,
    fontLabel,
    type SystemFont,
  } from "$lib/terminal-font";

  let {
    fonts,
    value,
    onSelect,
    disabled = false,
    id = "terminal-font",
  }: {
    fonts: SystemFont[];
    value: string | null;
    onSelect: (family: string | null) => void;
    disabled?: boolean;
    id?: string;
  } = $props();

  let open = $state(false);
  let query = $state("");
  let highlight = $state(0);
  let root: HTMLDivElement | undefined;
  let list = $state<HTMLUListElement | undefined>(undefined);

  interface Item {
    family: string | null;
    label: string;
    previewFamily: string;
  }

  let items = $derived.by((): Item[] => {
    const needle = query.trim().toLowerCase();
    const matchesDefault = !needle || DEFAULT_FONT_LABEL.toLowerCase().includes(needle);
    const found = filterFonts(fonts, query).map((font) => ({
      family: font.family,
      label: font.family,
      previewFamily: font.family,
    }));

    if (!matchesDefault) return found;

    return [
      {
        family: null,
        label: DEFAULT_FONT_LABEL,
        previewFamily: "ui-monospace, monospace",
      },
      ...found,
    ];
  });

  let display = $derived(open ? query : fontLabel(value));
  let listId = $derived(`${id}-list`);
  let activeId = $derived(items[highlight] ? `${id}-option-${highlight}` : undefined);

  function openList(): void {
    if (disabled) return;
    open = true;
    query = "";
    if (value === null) {
      highlight = 0;
      return;
    }

    const index = fonts.findIndex((font) => font.family === value);
    highlight = index === -1 ? 0 : index + 1;
  }

  function closeList(): void {
    open = false;
    query = "";
    highlight = 0;
  }

  function choose(family: string | null): void {
    onSelect(family);
    closeList();
  }

  function onInput(event: Event): void {
    const next = (event.currentTarget as HTMLInputElement).value;
    query = next;
    open = true;
    highlight = 0;
  }

  function onKeydown(event: KeyboardEvent): void {
    if (disabled) return;

    if (event.key === "ArrowDown") {
      event.preventDefault();
      if (!open) {
        openList();
        return;
      }
      highlight = items.length === 0 ? 0 : (highlight + 1) % items.length;
      scrollHighlighted();
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) {
        openList();
        return;
      }
      highlight = items.length === 0 ? 0 : (highlight - 1 + items.length) % items.length;
      scrollHighlighted();
      return;
    }

    if (event.key === "Enter") {
      if (!open || items.length === 0) return;
      event.preventDefault();
      choose(items[highlight]?.family ?? null);
      return;
    }

    if (event.key === "Escape") {
      if (!open) return;
      event.preventDefault();
      closeList();
    }
  }

  function scrollHighlighted(): void {
    const option = list?.querySelector<HTMLElement>(`#${id}-option-${highlight}`);
    option?.scrollIntoView({ block: "nearest" });
  }

  function onWindowPointerDown(event: PointerEvent): void {
    if (!open || !root) return;
    const target = event.target;
    if (target instanceof Node && root.contains(target)) return;
    closeList();
  }
</script>

<svelte:window onpointerdown={onWindowPointerDown} />

<div class="combo" bind:this={root}>
  <input
    {id}
    class="field"
    role="combobox"
    aria-autocomplete="list"
    aria-expanded={open}
    aria-controls={listId}
    aria-activedescendant={activeId}
    autocomplete="off"
    spellcheck="false"
    placeholder={fontLabel(value)}
    {disabled}
    value={display}
    onfocus={openList}
    oninput={onInput}
    onkeydown={onKeydown}
  />

  {#if open}
    <ul class="list" id={listId} role="listbox" bind:this={list}>
      {#if items.length === 0}
        <li class="empty" role="presentation">No hay fuentes que coincidan.</li>
      {:else}
        {#each items as item, index (item.family ?? "__default")}
          <li
            id={`${id}-option-${index}`}
            class={["option", { active: index === highlight, selected: item.family === value }]}
            role="option"
            aria-selected={item.family === value}
            style:font-family={item.previewFamily}
            onpointerdown={(event) => {
              event.preventDefault();
              choose(item.family);
            }}
            onpointerenter={() => {
              highlight = index;
            }}
          >
            {item.label}
          </li>
        {/each}
      {/if}
    </ul>
  {/if}
</div>

<style>
  .combo {
    position: relative;
    width: min(32rem, 100%);
  }

  .field {
    box-sizing: border-box;
    width: 100%;
    padding: 0.5rem 0.7rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.9rem;
  }

  .field:focus {
    outline: none;
    border-color: var(--accent);
  }

  .field:disabled {
    color: var(--text-faint);
    cursor: not-allowed;
  }

  .list {
    position: absolute;
    z-index: 20;
    top: calc(100% + 0.3rem);
    right: 0;
    left: 0;
    max-height: 16rem;
    margin: 0;
    padding: 0.25rem;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    list-style: none;
    box-shadow: 0 10px 28px #00000055;
  }

  .option,
  .empty {
    padding: 0.4rem 0.55rem;
    border-radius: 4px;
    color: var(--text);
    font-size: 0.88rem;
    line-height: 1.35;
  }

  .empty {
    color: var(--text-faint);
  }

  .option {
    cursor: pointer;
  }

  .option.active {
    background: var(--surface-hover);
  }

  .option.selected {
    color: var(--accent);
  }
</style>
