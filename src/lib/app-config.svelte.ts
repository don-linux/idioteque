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

  async load(): Promise<void> {
    try {
      const config = await invoke<AppConfigDto>("load_app_config");
      this.recents = config.recents;
      this.error = null;
    } catch (error) {
      this.recents = [];
      this.error = messageFrom(error);
    } finally {
      this.loaded = true;
    }
  }

  async record(path: string): Promise<void> {
    try {
      const config = await invoke<AppConfigDto>("record_recent_folder", { path });
      this.recents = config.recents;
      this.error = null;
    } catch (error) {
      this.error = messageFrom(error);
    }
  }

  async remove(path: string): Promise<void> {
    try {
      const config = await invoke<AppConfigDto>("remove_recent_folder", { path });
      this.recents = config.recents;
      this.error = null;
    } catch (error) {
      this.error = messageFrom(error);
    }
  }
}

export const appConfig = new AppConfig();
