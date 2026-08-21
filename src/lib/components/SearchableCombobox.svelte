<script lang="ts">
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import { comboItemLabel, filterComboItems, type ComboItem } from "$lib/combobox";

  let {
    items,
    value,
    onSelect,
    disabled = false,
    id,
    emptyLabel = "No hay coincidencias.",
  }: {
    items: ComboItem[];
    value: string | null;
    onSelect: (value: string | null) => void;
    disabled?: boolean;
    id: string;
    emptyLabel?: string;
  } = $props();

  let open = $state(false);
  let query = $state("");
  let highlight = $state(0);
  let root: HTMLDivElement | undefined;
  let list = $state<HTMLUListElement | undefined>(undefined);

  let visible = $derived(filterComboItems(items, query));
  let display = $derived(open ? query : comboItemLabel(items, value));
  let listId = $derived(`${id}-list`);
  let activeId = $derived(visible[highlight] ? `${id}-option-${highlight}` : undefined);

  function openList(): void {
    if (disabled) return;
    open = true;
    query = "";
    const index = items.findIndex((item) => item.value === value);
    highlight = index === -1 ? 0 : index;
  }

  function closeList(): void {
    open = false;
    query = "";
    highlight = 0;
  }

  function choose(next: string | null): void {
    onSelect(next);
    closeList();
  }

  function onInput(event: Event): void {
    query = (event.currentTarget as HTMLInputElement).value;
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
      highlight = visible.length === 0 ? 0 : (highlight + 1) % visible.length;
      scrollHighlighted();
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) {
        openList();
        return;
      }
      highlight = visible.length === 0 ? 0 : (highlight - 1 + visible.length) % visible.length;
      scrollHighlighted();
      return;
    }

    if (event.key === "Enter") {
      if (!open || visible.length === 0) return;
      event.preventDefault();
      choose(visible[highlight]?.value ?? null);
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
  <div class="field-wrap">
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
      placeholder={comboItemLabel(items, value)}
      {disabled}
      value={display}
      onfocus={openList}
      oninput={onInput}
      onkeydown={onKeydown}
    />
    <span class={["chevron", { open }]} aria-hidden="true">
      <ChevronDown size={16} strokeWidth={1.75} />
    </span>
  </div>

  {#if open}
    <ul class="list" id={listId} role="listbox" bind:this={list}>
      {#if visible.length === 0}
        <li class="empty" role="presentation">{emptyLabel}</li>
      {:else}
        {#each visible as item, index (item.value ?? "__default")}
          <li
            id={`${id}-option-${index}`}
            class={["option", { active: index === highlight, selected: item.value === value }]}
            role="option"
            aria-selected={item.value === value}
            style:font-family={item.previewFamily}
            onpointerdown={(event) => {
              event.preventDefault();
              choose(item.value);
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

  .field-wrap {
    position: relative;
  }

  .field {
    box-sizing: border-box;
    width: 100%;
    padding: 0.5rem 2.1rem 0.5rem 0.7rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.9rem;
  }

  .chevron {
    position: absolute;
    top: 50%;
    right: 0.55rem;
    display: inline-flex;
    color: var(--text-muted);
    pointer-events: none;
    transform: translateY(-50%);
  }

  .chevron.open {
    transform: translateY(-50%) rotate(180deg);
  }

  .field-wrap:has(.field:disabled) .chevron {
    color: var(--text-faint);
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
    box-shadow: 0 10px 28px var(--shadow);
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
