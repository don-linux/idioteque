<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import { page } from "$app/state";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import Save from "@lucide/svelte/icons/save";
  import { settingsBackHref } from "$lib/app-routes";
  import { handleSaveShortcut } from "$lib/save-shortcut";
  import { settingsEditor } from "$lib/settings-editor.svelte";
  import { SETTINGS_SECTIONS, settingsSectionFromPath } from "$lib/settings-sections";
  import { workspace } from "$lib/workspace.svelte";

  let { children }: { children: Snippet } = $props();
  let current = $derived(settingsSectionFromPath(page.url.pathname));
  let backHref = $derived(settingsBackHref(workspace.root !== null));

  onMount(() => {
    settingsEditor.begin();
    return () => settingsEditor.discard();
  });

  function onWindowKeydown(event: KeyboardEvent): void {
    handleSaveShortcut(event, { save: () => void settingsEditor.save() });
  }
</script>

<svelte:head>
  <title>configuración — idioteque</title>
</svelte:head>

<svelte:window onkeydown={onWindowKeydown} />

<main class="settings">
  <header>
    <a href={backHref} class="back" aria-label="Volver">
      <ArrowLeft size={18} strokeWidth={1.75} aria-hidden="true" />
    </a>
    <h1>Configuración</h1>
    <button
      type="button"
      class="save"
      aria-label="Guardar configuración (Ctrl+S)"
      title="Guardar configuración (Ctrl+S)"
      disabled={!settingsEditor.dirty || settingsEditor.saving}
      onclick={() => void settingsEditor.save()}
    >
      <Save size={18} strokeWidth={1.75} aria-hidden="true" />
      Guardar configuración
    </button>
  </header>

  <div class="body">
    <aside>
      <nav aria-label="Secciones">
        {#each SETTINGS_SECTIONS as section (section.id)}
          <a
            href={section.href}
            class={["item", { active: current?.id === section.id }]}
            aria-current={current?.id === section.id ? "page" : undefined}
          >
            {section.label}
          </a>
        {/each}
      </nav>
    </aside>
    <section class="content">
      {@render children()}
    </section>
  </div>
</main>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  header {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    gap: 0.75rem;
    padding: 2rem 2rem 1.25rem;
    border-bottom: 1px solid var(--border);
  }

  h1 {
    margin: 0;
    font-size: 1.6rem;
    font-weight: 600;
  }

  .back {
    display: inline-flex;
    box-sizing: border-box;
    flex-shrink: 0;
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

  .back:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .save {
    display: inline-flex;
    box-sizing: border-box;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    margin-left: auto;
    height: 2.5rem;
    padding: 0.55rem 1rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface-hover);
    color: var(--text);
    font: inherit;
    font-size: 0.95rem;
    font-weight: 600;
    cursor: pointer;
  }

  .save:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }

  .save:disabled {
    color: var(--text-faint);
    cursor: not-allowed;
  }

  .body {
    display: grid;
    grid-template-columns: 15rem 1fr;
    flex: 1;
    min-height: 0;
  }

  aside {
    min-width: 0;
    min-height: 0;
    overflow: auto;
    border-right: 1px solid var(--border);
    background: var(--surface);
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    padding: 0.5rem;
  }

  .item {
    padding: 0.45rem 0.65rem;
    border-radius: 4px;
    color: var(--text-muted);
    font-size: 0.9rem;
    text-decoration: none;
  }

  .item:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .item.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .content {
    min-width: 0;
    min-height: 0;
    overflow: auto;
  }
</style>
