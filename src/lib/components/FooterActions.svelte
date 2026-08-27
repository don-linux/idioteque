<script lang="ts">
  import { flip } from "svelte/animate";
  import Folder from "@lucide/svelte/icons/folder";
  import GitBranch from "@lucide/svelte/icons/git-branch";
  import House from "@lucide/svelte/icons/house";
  import Settings from "@lucide/svelte/icons/settings";
  import SquareTerminal from "@lucide/svelte/icons/square-terminal";
  import { goto } from "$app/navigation";
  import { appConfig } from "$lib/app-config.svelte";
  import { ROUTES } from "$lib/app-routes";
  import { moveFooterAction, runFooterAction, type FooterActionId } from "$lib/footer-actions";
  import { gitStatus } from "$lib/git";
  import {
    gitFooterStateFromError,
    gitFooterStateFromSnapshot,
    gitFooterTitle,
    type GitFooterState,
  } from "$lib/git-footer";
  import { dockFromAlt } from "$lib/terminal-dock";
  import { terminal } from "$lib/terminal.svelte";
  import { workspace } from "$lib/workspace.svelte";

  const DRAG_THRESHOLD_PX = 4;

  let dragging = $state<FooterActionId | null>(null);
  let dragMoved = $state(false);
  let startX = 0;

  const labels: Record<FooterActionId, string> = {
    home: "Inicio",
    folder: "Cambiar",
    settings: "Configuración",
    terminal: "Terminal",
    git: "Git",
  };

  const titles: Record<FooterActionId, string> = {
    home: "Inicio",
    folder: "Cambiar carpeta",
    settings: "Configuración",
    terminal: "Terminal (Ctrl+J) · a la derecha (Ctrl+Alt+J) · pantalla (Ctrl+Shift+J)",
    git: "Git",
  };

  let gitState = $state.raw<GitFooterState>({ kind: "loading" });
  let gitGen = 0;

  async function refreshGit(root: string | null): Promise<void> {
    if (!root) {
      gitState = { kind: "empty" };
      return;
    }
    const gen = ++gitGen;
    try {
      const snap = await gitStatus(root);
      if (gen !== gitGen) return;
      gitState = gitFooterStateFromSnapshot(snap);
    } catch {
      if (gen !== gitGen) return;
      gitState = gitFooterStateFromError();
    }
  }

  $effect(() => {
    const root = workspace.root;
    void refreshGit(root);
  });

  let gitTitle = $derived(gitFooterTitle(gitState));

  function startDrag(id: FooterActionId, event: PointerEvent): void {
    if (event.button !== 0) return;

    dragging = id;
    dragMoved = false;
    startX = event.clientX;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function onPointerMove(event: PointerEvent): void {
    if (dragging === null) return;

    if (!dragMoved && Math.abs(event.clientX - startX) < DRAG_THRESHOLD_PX) {
      return;
    }

    dragMoved = true;

    const over = document
      .elementFromPoint(event.clientX, event.clientY)
      ?.closest("[data-footer-action]");
    const overId = over?.getAttribute("data-footer-action");
    if (!overId || overId === dragging) return;

    const from = appConfig.footerOrder.indexOf(dragging);
    const to = appConfig.footerOrder.indexOf(overId as FooterActionId);
    if (from < 0 || to < 0 || from === to) return;

    appConfig.footerOrder = moveFooterAction(appConfig.footerOrder, from, to);
  }

  function endDrag(): void {
    if (dragging === null) return;

    const moved = dragMoved;
    dragging = null;

    if (moved) {
      void appConfig.saveFooterOrder(appConfig.footerOrder);
      setTimeout(() => {
        dragMoved = false;
      }, 0);
    }
  }

  function onActionClick(id: FooterActionId, event: MouseEvent): void {
    if (dragMoved) {
      event.preventDefault();
      return;
    }

    runFooterAction(id, {
      home: () => {
        void workspace.closeWorkspace().then((left) => {
          if (left) void goto(ROUTES.home);
        });
      },
      folder: () => {
        void workspace.openFolder();
      },
      terminal: () => {
        terminal.toggle(dockFromAlt(event.altKey));
      },
    });
  }
</script>

<div class="actions">
  {#each appConfig.footerOrder as id (id)}
    <span
      class={["item", { dragging: dragging === id }]}
      animate:flip={{ duration: 160 }}
      data-footer-action={id}
    >
      {#if id === "settings"}
        <a
          href={ROUTES.settings}
          class="action"
          aria-label={labels.settings}
          title={titles.settings}
          draggable="false"
          onpointerdown={(event) => startDrag(id, event)}
          onpointermove={onPointerMove}
          onpointerup={endDrag}
          onpointercancel={endDrag}
          onclick={(event) => onActionClick(id, event)}
        >
          <Settings size={16} strokeWidth={1.75} aria-hidden="true" />
        </a>
      {:else}
        <button
          type="button"
          class={[
            "action",
            { active: id === "terminal" && (terminal.open || terminal.surface === "terminals") },
          ]}
          aria-pressed={id === "terminal"
            ? terminal.open || terminal.surface === "terminals"
            : undefined}
          aria-label={labels[id]}
          title={id === "git" ? gitTitle : titles[id]}
          onpointerenter={() => {
            if (id === "git") void refreshGit(workspace.root);
          }}
          onpointerdown={(event) => startDrag(id, event)}
          onpointermove={onPointerMove}
          onpointerup={endDrag}
          onpointercancel={endDrag}
          onclick={(event) => onActionClick(id, event)}
        >
          {#if id === "home"}
            <House size={16} strokeWidth={1.75} aria-hidden="true" />
          {:else if id === "folder"}
            <Folder size={16} strokeWidth={1.75} aria-hidden="true" />
          {:else if id === "git"}
            <GitBranch size={16} strokeWidth={1.75} aria-hidden="true" />
          {:else}
            <SquareTerminal size={16} strokeWidth={1.75} aria-hidden="true" />
          {/if}
        </button>
      {/if}
    </span>
  {/each}
</div>

<style>
  .actions {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    margin-left: auto;
    gap: 0.15rem;
    user-select: none;
  }

  .item {
    display: inline-flex;
  }

  .item.dragging {
    pointer-events: none;
    opacity: 0.55;
  }

  .action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.65rem;
    height: 1.65rem;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-muted);
    text-decoration: none;
    cursor: grab;
    touch-action: none;
  }

  .action:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .action.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .actions:has(.dragging) .action {
    cursor: grabbing;
  }
</style>
