import { invoke } from "@tauri-apps/api/core";
import {
  clampFontSize,
  DEFAULT_TERMINAL_FONT_SIZE,
  type SystemFont,
} from "$lib/terminal-font";

export interface RecentFolder {
  path: string;
  openedAt: string;
  exists: boolean;
}

export interface TerminalSettings {
  fontFamily: string | null;
  fontSize: number;
}

interface AppConfigDto {
  version: number;
  recents: RecentFolder[];
  terminal: TerminalSettings;
}

function messageFrom(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

function normalizeFamily(family: string | null | undefined): string | null {
  const trimmed = family?.trim();
  return trimmed ? trimmed : null;
}

class AppConfig {
  recents = $state<RecentFolder[]>([]);
  terminalFontFamily = $state<string | null>(null);
  terminalFontSize = $state(DEFAULT_TERMINAL_FONT_SIZE);
  fonts = $state.raw<SystemFont[]>([]);
  fontsLoaded = $state(false);
  loaded = $state(false);
  error = $state<string | null>(null);
  /// Drops stale load/record/remove responses that finish out of order.
  #gen = 0;

  apply(config: AppConfigDto): void {
    this.recents = config.recents;
    this.terminalFontFamily = normalizeFamily(config.terminal.fontFamily);
    this.terminalFontSize = clampFontSize(config.terminal.fontSize);
    this.error = null;
  }

  async load(): Promise<void> {
    const gen = ++this.#gen;

    try {
      const config = await invoke<AppConfigDto>("load_app_config");
      if (gen !== this.#gen) return;
      this.apply(config);
    } catch (error) {
      if (gen !== this.#gen) return;
      this.recents = [];
      this.terminalFontFamily = null;
      this.terminalFontSize = DEFAULT_TERMINAL_FONT_SIZE;
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
      this.apply(config);
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
      this.apply(config);
    } catch (error) {
      if (gen !== this.#gen) return;
      this.error = messageFrom(error);
    }
  }

  async saveTerminal(input: { fontFamily: string | null; fontSize: number }): Promise<void> {
    const fontFamily = normalizeFamily(input.fontFamily);
    const fontSize = clampFontSize(input.fontSize);
    this.terminalFontFamily = fontFamily;
    this.terminalFontSize = fontSize;

    const gen = ++this.#gen;

    try {
      const config = await invoke<AppConfigDto>("update_terminal_settings", {
        terminal: { fontFamily, fontSize },
      });
      if (gen !== this.#gen) return;
      this.apply(config);
    } catch (error) {
      if (gen !== this.#gen) return;
      this.error = messageFrom(error);
    }
  }

  async listSystemFonts(): Promise<SystemFont[]> {
    if (this.fontsLoaded) return this.fonts;

    try {
      const fonts = await invoke<SystemFont[]>("list_system_fonts");
      this.fonts = fonts;
      this.fontsLoaded = true;
      return fonts;
    } catch (error) {
      this.fonts = [];
      this.fontsLoaded = true;
      this.error = messageFrom(error);
      return [];
    }
  }
}

export const appConfig = new AppConfig();
