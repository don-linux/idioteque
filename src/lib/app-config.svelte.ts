import { invoke } from "@tauri-apps/api/core";

export interface RecentFolder {
  path: string;
  openedAt: string;
  exists: boolean;
}

interface AppConfigDto {
  version: number;
  recents: RecentFolder[];
}

function messageFrom(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

class AppConfig {
  recents = $state<RecentFolder[]>([]);
  loaded = $state(false);
  error = $state<string | null>(null);
  /// Drops stale load/record/remove responses that finish out of order.
  #gen = 0;

  async load(): Promise<void> {
    const gen = ++this.#gen;

    try {
      const config = await invoke<AppConfigDto>("load_app_config");
      if (gen !== this.#gen) return;
      this.recents = config.recents;
      this.error = null;
    } catch (error) {
      if (gen !== this.#gen) return;
      this.recents = [];
      this.error = messageFrom(error);
    } finally {
      if (gen === this.#gen) {
        this.loaded = true;
      }
    }
  }

  async record(path: string): Promise<string | null> {
    const gen = ++this.#gen;

    try {
      const config = await invoke<AppConfigDto>("record_recent_folder", { path });
      const recorded = config.recents[0]?.path ?? null;
      if (gen !== this.#gen) return recorded;
      this.recents = config.recents;
      this.error = null;
      return recorded;
    } catch (error) {
      if (gen !== this.#gen) return null;
      this.error = messageFrom(error);
      return null;
    }
  }

  async remove(path: string): Promise<void> {
    const gen = ++this.#gen;

    try {
      const config = await invoke<AppConfigDto>("remove_recent_folder", { path });
      if (gen !== this.#gen) return;
      this.recents = config.recents;
      this.error = null;
    } catch (error) {
      if (gen !== this.#gen) return;
      this.error = messageFrom(error);
    }
  }
}

export const appConfig = new AppConfig();
