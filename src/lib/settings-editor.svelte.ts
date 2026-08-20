import { appConfig } from "$lib/app-config.svelte";
import { clampFontSize, DEFAULT_TERMINAL_FONT_SIZE } from "$lib/terminal-font";
import { isSettingsDirty, normalizeFontFamily } from "$lib/settings-draft";
import { SETTINGS_SAVED_TOAST } from "$lib/toast";
import { toasts } from "$lib/toast.svelte";

class SettingsEditor {
  draftFamily = $state<string | null>(null);
  draftSize = $state(DEFAULT_TERMINAL_FONT_SIZE);
  dirty = $state(false);
  saving = $state(false);

  get fontFamily(): string | null {
    return this.dirty ? this.draftFamily : appConfig.terminalFontFamily;
  }

  get fontSize(): number {
    return this.dirty ? this.draftSize : appConfig.terminalFontSize;
  }

  begin(): void {
    this.dirty = false;
  }

  discard(): void {
    this.dirty = false;
  }

  setFont(family: string | null): void {
    this.#commit({
      fontFamily: normalizeFontFamily(family),
      fontSize: this.fontSize,
    });
  }

  setSize(size: number): void {
    this.#commit({
      fontFamily: this.fontFamily,
      fontSize: clampFontSize(size),
    });
  }

  async save(): Promise<boolean> {
    if (!this.dirty || this.saving) return false;

    this.saving = true;

    try {
      await appConfig.saveTerminal({
        fontFamily: this.draftFamily,
        fontSize: this.draftSize,
      });

      if (appConfig.error) return false;

      this.dirty = false;
      toasts.success(SETTINGS_SAVED_TOAST);
      return true;
    } finally {
      this.saving = false;
    }
  }

  #commit(draft: { fontFamily: string | null; fontSize: number }): void {
    this.draftFamily = draft.fontFamily;
    this.draftSize = draft.fontSize;
    this.dirty = isSettingsDirty(draft, {
      fontFamily: appConfig.terminalFontFamily,
      fontSize: appConfig.terminalFontSize,
    });
  }
}

export const settingsEditor = new SettingsEditor();
