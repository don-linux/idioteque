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
pub mod state;
pub mod updater;
pub mod version;
