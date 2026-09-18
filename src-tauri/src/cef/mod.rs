//! Navegador CEF: el ADE solo spawnea y habla con `cef-host`. Ver `docs/cef/CONTRACT.md`.

pub mod archive;
pub mod denylist;
pub mod download;
pub mod elf_strip;
pub mod health;
pub mod host;
pub mod index;
pub mod ipc;
pub mod manifest;
pub mod paths;
pub mod promote;
pub mod sandbox;
pub mod state;
pub mod updater;
pub mod version;

#[allow(unused_imports)]
pub use host::{
    browser_command, browser_kill, browser_set_bounds, browser_set_visible, browser_spawn,
    kill_on_exit, spawn_host, Bounds, BrowserBoot, CefState, HostLaunch, HostProcess,
};
#[allow(unused_imports)]
pub use ipc::{encode_command, parse_event, HostCommand, HostEvent};
#[allow(unused_imports)]
pub use manifest::{
    load, resolve_effective, save, slot_info, validate, EffectiveSlot, EffectiveSource,
    ManifestFile, SlotInfo, SlotManifest, SlotSource, REQUIRED_FILES_LINUX64,
};
#[allow(unused_imports)]
pub use paths::{base_info, host_binary_path, BaseInfo, CefPaths, PLATFORM};
#[allow(unused_imports)]
pub use version::{chromium_from, CefVersion};
