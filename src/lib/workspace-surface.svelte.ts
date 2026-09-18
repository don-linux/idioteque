import type { WorkspaceSurface } from "$lib/workspace-surface";

export type { WorkspaceSurface };

class WorkspaceSurfaceState {
  current = $state<WorkspaceSurface>("editor");

  set(next: WorkspaceSurface): void {
    this.current = next;
  }
}

export const surface = new WorkspaceSurfaceState();
