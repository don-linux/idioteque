import {
  gitGraphLog,
  gitRefs,
  type GitCommit,
  type GitComparison,
  type GitRef,
} from "$lib/git";

/**
 * Graph panel state lives outside the component so hiding the sidebar
 * does not forget which branches were compared.
 */
class GitGraphView {
  branches = $state<GitRef[]>([]);
  current = $state<string | null>(null);
  oid = $state<string | null>(null);
  detached = $state(false);
  initial = $state(false);
  empty = $state(false);
  unavailable = $state(false);
  loading = $state(false);
  /** Extra local branches marked in the picker. The current branch is always included. */
  selected = $state<string[]>([]);
  commits = $state<GitCommit[]>([]);
  comparisons = $state<GitComparison[]>([]);
  error = $state<string | null>(null);

  #root: string | null = null;
  #gen = 0;

  resetSelection(): void {
    const changed = this.selected.length > 0;
    this.selected = [];
    if (changed && this.#root) void this.refresh();
  }

  isSelected(name: string): boolean {
    if (this.current && name === this.current) return true;
    return this.selected.includes(name);
  }

  toggleBranch(name: string): void {
    if (this.current && name === this.current) return;

    if (this.selected.includes(name)) {
      this.selected = this.selected.filter((branch) => branch !== name);
    } else {
      this.selected = [...this.selected, name];
    }

    void this.refresh();
  }

  async setRoot(root: string | null): Promise<void> {
    if (root === this.#root) return;
    this.#root = root;
    this.resetSelection();
    await this.refresh();
  }

  async refresh(): Promise<void> {
    const root = this.#root;
    const gen = ++this.#gen;

    if (!root) {
      this.#clear(false);
      return;
    }

    this.loading = true;
    this.error = null;

    try {
      const refs = await gitRefs(root);
      if (gen !== this.#gen) return;

      if (!refs.probe.available) {
        this.#clear(true);
        this.unavailable = true;
        return;
      }

      const repo = refs.repository;
      if (!repo) {
        this.#clear(false);
        this.empty = true;
        return;
      }

      this.branches = repo.branches;
      this.current = repo.current ?? null;
      this.oid = repo.oid ?? null;
      this.detached = repo.detached;
      this.initial = repo.initial;
      this.empty = false;
      this.unavailable = false;
      this.selected = this.selected.filter(
        (name) => name !== this.current && repo.branches.some((branch) => branch.name === name),
      );

      if (repo.initial) {
        this.commits = [];
        this.comparisons = [];
        this.loading = false;
        return;
      }

      const graph = await gitGraphLog(root, this.selected);
      if (gen !== this.#gen) return;

      this.commits = graph.repository?.commits ?? [];
      this.comparisons = graph.repository?.comparisons ?? [];
      this.loading = false;
    } catch {
      if (gen !== this.#gen) return;
      this.#clear(false);
      this.error = "Git no responde";
    }
  }

  #clear(unavailable: boolean): void {
    this.branches = [];
    this.current = null;
    this.oid = null;
    this.detached = false;
    this.initial = false;
    this.empty = !unavailable;
    this.unavailable = unavailable;
    this.commits = [];
    this.comparisons = [];
    this.loading = false;
    this.error = null;
  }
}

export const gitGraph = new GitGraphView();
