export const SETTINGS_SECTIONS = [
  { id: "terminal", label: "Terminal", href: "/configuracion/terminal" },
] as const;

export type SettingsSection = (typeof SETTINGS_SECTIONS)[number];
export type SettingsSectionId = SettingsSection["id"];

export function settingsSectionFromPath(pathname: string): SettingsSection | null {
  return SETTINGS_SECTIONS.find((section) => section.href === pathname) ?? null;
}
