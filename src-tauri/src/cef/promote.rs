//! Promoción atómica `candidate/` → `current/` y recuperación al arrancar (contrato 8).

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use super::manifest::{self, SlotManifest};
use super::paths::CefPaths;
use super::state::{self, PendingPromotion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Promoted {
    pub cef_version: String,
    pub chromium_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromoteResult {
    Promoted(Promoted),
    Deferred(PendingPromotion),
    NothingToPromote,
}

pub fn mark_candidate_verified(paths: &CefPaths) -> Result<(), String> {
    let dir = paths.candidate();
    let mut manifest = manifest::load(&dir)?;
    manifest.verified = true;
    manifest.verified_at = Some(state::now_rfc3339());
    manifest::save(&dir, &manifest)
}

pub fn discard_candidate(paths: &CefPaths) -> Result<(), String> {
    remove_dir_if_exists(&paths.candidate())
}

pub fn promote_candidate(paths: &CefPaths, host_alive: bool) -> Result<PromoteResult, String> {
    let candidate = paths.candidate();
    if !candidate.exists() {
        return Ok(PromoteResult::NothingToPromote);
    }

    let manifest = match manifest::load(&candidate) {
        Ok(manifest) => manifest,
        Err(error) => {
            if candidate.join("manifest.json").is_file() {
                return Err(error);
            }
            return Ok(PromoteResult::NothingToPromote);
        }
    };

    if !manifest.verified {
        return Err("El candidate no está verificado; no se puede promover".to_string());
    }
    manifest::validate(&candidate, &manifest)?;

    let pending = PendingPromotion {
        cef_version: manifest.cef_version.clone(),
        chromium_version: manifest.chromium_version.clone(),
    };

    if host_alive {
        let mut updater = state::load(paths);
        updater.pending_promotion = Some(pending.clone());
        state::save(paths, &updater)?;
        return Ok(PromoteResult::Deferred(pending));
    }

    swap_candidate_into_current(paths, manifest)
}

pub fn recover_at_startup(paths: &CefPaths, host_alive: bool) -> Result<Option<Promoted>, String> {
    // CONTRACT 8: current ausente + current.old → restaurar. Si current existe
    // pero es inválido y current.old es last-good, también se restaura: un
    // rename a medias o un slot roto no debe tapar el motor que sí arranca.
    recover_current_slot(paths)?;

    let mut promoted = None;
    let candidate = paths.candidate();
    if candidate.exists() {
        if candidate_is_valid_verified(&candidate) {
            match promote_candidate(paths, host_alive)? {
                PromoteResult::Promoted(done) => promoted = Some(done),
                PromoteResult::Deferred(_) | PromoteResult::NothingToPromote => {}
            }
        } else {
            discard_candidate(paths)?;
        }
    }

    clear_stale_pending(paths)?;
    state::remove_health_caches(paths);
    Ok(promoted)
}

fn swap_candidate_into_current(
    paths: &CefPaths,
    mut manifest: SlotManifest,
) -> Result<PromoteResult, String> {
    let current = paths.current();
    let current_old = paths.current_old();
    let candidate = paths.candidate();

    recover_current_slot(paths)?;

    if current_old.exists() {
        remove_path_if_exists(&current_old)?;
    }

    if current.exists() {
        fs::rename(&current, &current_old).map_err(|error| {
            format!(
                "No se pudo mover `{}` a `{}`: {error}",
                current.display(),
                current_old.display()
            )
        })?;
    }

    fs::rename(&candidate, &current).map_err(|error| {
        format!(
            "No se pudo promover `{}` a `{}`: {error}",
            candidate.display(),
            current.display()
        )
    })?;

    let _ = fs::remove_dir_all(&current_old);

    manifest.verified = false;
    manifest::save(&current, &manifest)?;

    let mut updater = state::load(paths);
    updater.pending_promotion = None;
    state::save(paths, &updater)?;

    Ok(PromoteResult::Promoted(Promoted {
        cef_version: manifest.cef_version,
        chromium_version: manifest.chromium_version,
    }))
}

fn candidate_is_valid_verified(dir: &Path) -> bool {
    match manifest::load(dir) {
        Ok(manifest) if manifest.verified => manifest::validate(dir, &manifest).is_ok(),
        _ => false,
    }
}

fn slot_is_valid(dir: &Path) -> bool {
    match manifest::load(dir) {
        Ok(manifest) => manifest::validate(dir, &manifest).is_ok(),
        Err(_) => false,
    }
}

fn recover_current_slot(paths: &CefPaths) -> Result<(), String> {
    let current = paths.current();
    let current_old = paths.current_old();
    let current_valid = slot_is_valid(&current);
    let old_valid = slot_is_valid(&current_old);

    if !current_valid {
        if old_valid {
            remove_path_if_exists(&current)?;
            fs::rename(&current_old, &current).map_err(|error| {
                format!(
                    "No se pudo restaurar `{}` a `{}`: {error}",
                    current_old.display(),
                    current.display()
                )
            })?;
        } else if current_old.exists() {
            // current.old no es last-good (dir incompleto, archivo suelto).
            // No se promociona a current: el arranque cae al base bundleado.
            remove_path_if_exists(&current_old)?;
        }
    }

    if current_old.exists() && slot_is_valid(&current) {
        remove_path_if_exists(&current_old)?;
    }
    Ok(())
}

fn clear_stale_pending(paths: &CefPaths) -> Result<(), String> {
    if candidate_is_valid_verified(&paths.candidate()) {
        return Ok(());
    }
    let mut updater = state::load(paths);
    if updater.pending_promotion.is_none() {
        return Ok(());
    }
    updater.pending_promotion = None;
    state::save(paths, &updater)
}

fn remove_dir_if_exists(path: &Path) -> Result<(), String> {
    remove_path_if_exists(path)
}

fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "No se pudo inspeccionar `{}`: {error}",
                path.display()
            ));
        }
    };
    let result = if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("No se pudo borrar `{}`: {error}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cef::manifest::{ManifestFile, SlotSource, REQUIRED_FILES_LINUX64};
    use crate::cef::paths::PLATFORM;
    use crate::cef::version::chromium_from;
    use tempfile::TempDir;

    const BUNDLED: &str = "152.0.6+g708dc14+chromium-152.0.7977.83";
    const NEWER: &str = "153.0.1+gabc+chromium-153.0.8000.10";
    const OLDER: &str = "151.0.1+gold+chromium-151.0.1.1";

    fn paths_in(tmp: &TempDir) -> CefPaths {
        CefPaths::new(tmp.path().join("home"), tmp.path().join("base"))
    }

    fn sample_manifest(version: &str, source: SlotSource) -> SlotManifest {
        let chromium = chromium_from(version).unwrap_or_else(|| "0.0.0.0".into());
        SlotManifest {
            schema: 1,
            cef_version: version.to_string(),
            chromium_version: chromium,
            platform: PLATFORM.to_string(),
            api_version_min: 13300,
            api_version_last: 15200,
            source,
            archive_name: "cef_binary_linux64_minimal.tar.bz2".into(),
            archive_sha1: "9711b86c105fb590da576fe5a829802f1a79d520".into(),
            archive_size: 321503907,
            stripped: true,
            files: Vec::new(),
            verified: false,
            verified_at: None,
            created_at: "2026-09-16T23:00:00Z".into(),
        }
    }

    fn write_required(dir: &Path, size: u64) -> Vec<ManifestFile> {
        let payload = vec![b'x'; size as usize];
        let mut files = Vec::new();
        for name in REQUIRED_FILES_LINUX64 {
            let path = dir.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent");
            }
            fs::write(&path, &payload).expect("write");
            files.push(ManifestFile {
                path: (*name).to_string(),
                size,
                sha256: "ab".repeat(32),
            });
        }
        files
    }

    fn write_slot(dir: &Path, version: &str, source: SlotSource, verified: bool) -> SlotManifest {
        fs::create_dir_all(dir).expect("slot");
        let mut manifest = sample_manifest(version, source);
        manifest.files = write_required(dir, 1);
        manifest.verified = verified;
        if verified {
            manifest.verified_at = Some("2026-09-16T23:00:00Z".into());
        }
        manifest::save(dir, &manifest).expect("save");
        manifest
    }

    fn setup(tmp: &TempDir) -> CefPaths {
        let paths = paths_in(tmp);
        paths.ensure_dirs().unwrap();
        write_slot(&paths.bundled_base, BUNDLED, SlotSource::Bundled, false);
        paths
    }

    fn set_pending(paths: &CefPaths, version: &str) {
        let mut updater = state::load(paths);
        updater.pending_promotion = Some(state::PendingPromotion {
            cef_version: version.to_string(),
            chromium_version: chromium_from(version).unwrap_or_else(|| "0.0.0.0".into()),
        });
        state::save(paths, &updater).expect("pending");
    }

    /// `rename(current, current.old)` done; `rename(candidate, current)` never ran.
    fn crash_after_current_to_old(paths: &CefPaths) {
        assert!(
            paths.current().is_dir(),
            "crash fixture needs a current slot"
        );
        fs::rename(paths.current(), paths.current_old()).expect("simulate mid-rename");
        assert!(!paths.current().exists());
        assert!(paths.current_old().is_dir());
    }

    fn break_required_file(dir: &Path) {
        fs::remove_file(dir.join("libcef.so")).expect("break slot");
    }

    #[test]
    fn nothing_to_promote_when_candidate_absent() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        let result = promote_candidate(&paths, false).unwrap();
        assert_eq!(result, PromoteResult::NothingToPromote);
    }

    #[test]
    fn mark_candidate_verified_sets_flags() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, false);
        mark_candidate_verified(&paths).unwrap();
        let loaded = manifest::load(&paths.candidate()).unwrap();
        assert!(loaded.verified);
        assert!(loaded.verified_at.is_some());
    }

    #[test]
    fn discard_candidate_ok_if_absent() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        discard_candidate(&paths).unwrap();
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, false);
        discard_candidate(&paths).unwrap();
        assert!(!paths.candidate().exists());
    }

    #[test]
    fn candidate_not_verified_is_err() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, false);
        let error = promote_candidate(&paths, false).unwrap_err();
        assert!(error.contains("no está verificado"));
        assert!(paths.candidate().exists());
    }

    #[test]
    fn verified_candidate_without_current_is_promoted() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);

        let result = promote_candidate(&paths, false).unwrap();
        match result {
            PromoteResult::Promoted(done) => {
                assert_eq!(done.cef_version, NEWER);
                assert_eq!(done.chromium_version, "153.0.8000.10");
            }
            other => panic!("expected Promoted, got {other:?}"),
        }

        assert!(!paths.candidate().exists());
        assert!(!paths.current_old().exists());
        let current = manifest::load(&paths.current()).unwrap();
        assert_eq!(current.cef_version, NEWER);
        assert!(!current.verified);
        assert_eq!(current.source, SlotSource::Downloaded);
        assert_eq!(state::load(&paths).pending_promotion, None);
    }

    #[test]
    fn verified_candidate_replaces_current_and_drops_old() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);

        let result = promote_candidate(&paths, false).unwrap();
        assert!(matches!(result, PromoteResult::Promoted(_)));
        assert!(!paths.candidate().exists());
        assert!(!paths.current_old().exists());
        let current = manifest::load(&paths.current()).unwrap();
        assert_eq!(current.cef_version, NEWER);
        assert!(!current.verified);
        assert_eq!(current.source, SlotSource::Downloaded);
    }

    #[test]
    fn host_alive_defers_and_sets_pending_promotion() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);

        let result = promote_candidate(&paths, true).unwrap();
        match result {
            PromoteResult::Deferred(pending) => {
                assert_eq!(pending.cef_version, NEWER);
                assert_eq!(pending.chromium_version, "153.0.8000.10");
            }
            other => panic!("expected Deferred, got {other:?}"),
        }
        assert!(paths.candidate().exists());
        assert!(!paths.current().exists());
        let state = state::load(&paths);
        let pending = state.pending_promotion.expect("pending");
        assert_eq!(pending.cef_version, NEWER);
        let candidate = manifest::load(&paths.candidate()).unwrap();
        assert!(candidate.verified);
    }

    #[test]
    fn recover_renames_orphan_current_old() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current_old(), NEWER, SlotSource::Downloaded, false);

        let promoted = recover_at_startup(&paths, false).unwrap();
        assert_eq!(promoted, None);
        assert!(paths.current().exists());
        assert!(!paths.current_old().exists());
        assert_eq!(manifest::load(&paths.current()).unwrap().cef_version, NEWER);
    }

    #[test]
    fn recover_deletes_unverified_candidate() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, false);

        let promoted = recover_at_startup(&paths, false).unwrap();
        assert_eq!(promoted, None);
        assert!(!paths.candidate().exists());
    }

    #[test]
    fn recover_promotes_verified_candidate() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);

        let promoted = recover_at_startup(&paths, false).unwrap();
        let done = promoted.expect("promoted");
        assert_eq!(done.cef_version, NEWER);
        assert!(!paths.candidate().exists());
        assert!(!paths.current_old().exists());
        assert_eq!(manifest::load(&paths.current()).unwrap().cef_version, NEWER);
    }

    #[test]
    fn recover_defers_verified_candidate_when_host_alive() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);

        let promoted = recover_at_startup(&paths, true).unwrap();
        assert_eq!(promoted, None);
        assert!(paths.candidate().exists());
        assert_eq!(
            manifest::load(&paths.current()).unwrap().cef_version,
            BUNDLED
        );
        let pending = state::load(&paths).pending_promotion.expect("pending");
        assert_eq!(pending.cef_version, NEWER);
        assert!(manifest::load(&paths.candidate()).unwrap().verified);
    }

    #[test]
    fn recover_drops_stray_health_cache() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        let cache = paths.home.join("health-cache-123");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("x"), b"1").unwrap();

        recover_at_startup(&paths, false).unwrap();
        assert!(!cache.exists());
        assert!(paths.profile().is_dir());
    }

    #[test]
    fn recover_deletes_current_old_when_current_is_valid() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.current_old(), NEWER, SlotSource::Downloaded, false);

        recover_at_startup(&paths, false).unwrap();
        assert!(!paths.current_old().exists());
        assert_eq!(
            manifest::load(&paths.current()).unwrap().cef_version,
            BUNDLED
        );
    }

    #[test]
    fn crash_mid_rename_recover_promotes_verified_candidate() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);
        set_pending(&paths, NEWER);
        crash_after_current_to_old(&paths);

        let promoted = recover_at_startup(&paths, false).unwrap();
        let done = promoted.expect("promoted after crash");
        assert_eq!(done.cef_version, NEWER);
        assert!(!paths.candidate().exists());
        assert!(!paths.current_old().exists());
        assert_eq!(manifest::load(&paths.current()).unwrap().cef_version, NEWER);
        assert_eq!(state::load(&paths).pending_promotion, None);
    }

    #[test]
    fn crash_mid_rename_recover_defers_when_host_alive() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);
        crash_after_current_to_old(&paths);

        let promoted = recover_at_startup(&paths, true).unwrap();
        assert_eq!(promoted, None);
        assert_eq!(
            manifest::load(&paths.current()).unwrap().cef_version,
            BUNDLED
        );
        assert!(paths.candidate().exists());
        assert!(!paths.current_old().exists());
        assert!(manifest::load(&paths.candidate()).unwrap().verified);
        let pending = state::load(&paths).pending_promotion.expect("pending");
        assert_eq!(pending.cef_version, NEWER);
    }

    #[test]
    fn crash_mid_rename_restore_then_discard_unverified_candidate() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, false);
        set_pending(&paths, NEWER);
        crash_after_current_to_old(&paths);

        let promoted = recover_at_startup(&paths, false).unwrap();
        assert_eq!(promoted, None);
        assert!(!paths.candidate().exists());
        assert!(!paths.current_old().exists());
        assert_eq!(
            manifest::load(&paths.current()).unwrap().cef_version,
            BUNDLED
        );
        assert_eq!(state::load(&paths).pending_promotion, None);
    }

    #[test]
    fn host_alive_defer_does_not_restore_or_swap() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);
        crash_after_current_to_old(&paths);

        let result = promote_candidate(&paths, true).unwrap();
        assert!(matches!(result, PromoteResult::Deferred(_)));
        assert!(!paths.current().exists());
        assert_eq!(
            manifest::load(&paths.current_old()).unwrap().cef_version,
            BUNDLED
        );
        assert!(paths.candidate().exists());
        assert!(manifest::load(&paths.candidate()).unwrap().verified);
    }

    #[test]
    fn current_and_current_old_keep_current_and_promote_candidate() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.current_old(), OLDER, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);

        let promoted = recover_at_startup(&paths, false).unwrap();
        let done = promoted.expect("promoted");
        assert_eq!(done.cef_version, NEWER);
        assert!(!paths.candidate().exists());
        assert!(!paths.current_old().exists());
        assert_eq!(manifest::load(&paths.current()).unwrap().cef_version, NEWER);
        assert_ne!(manifest::load(&paths.current()).unwrap().cef_version, OLDER);
    }

    #[test]
    fn current_and_current_old_defer_does_not_swap_when_host_alive() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.current_old(), OLDER, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);

        let promoted = recover_at_startup(&paths, true).unwrap();
        assert_eq!(promoted, None);
        assert_eq!(
            manifest::load(&paths.current()).unwrap().cef_version,
            BUNDLED
        );
        assert!(!paths.current_old().exists());
        assert!(paths.candidate().exists());
        let pending = state::load(&paths).pending_promotion.expect("pending");
        assert_eq!(pending.cef_version, NEWER);
    }

    #[test]
    fn host_alive_defer_leaves_current_old_untouched() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.current_old(), OLDER, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);

        let result = promote_candidate(&paths, true).unwrap();
        assert!(matches!(result, PromoteResult::Deferred(_)));
        assert_eq!(
            manifest::load(&paths.current()).unwrap().cef_version,
            BUNDLED
        );
        assert_eq!(
            manifest::load(&paths.current_old()).unwrap().cef_version,
            OLDER
        );
        assert!(paths.candidate().exists());
    }

    #[test]
    fn leftover_current_old_does_not_block_promote() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.current_old(), OLDER, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);

        let result = promote_candidate(&paths, false).unwrap();
        assert!(matches!(result, PromoteResult::Promoted(_)));
        assert!(!paths.candidate().exists());
        assert!(!paths.current_old().exists());
        assert_eq!(manifest::load(&paths.current()).unwrap().cef_version, NEWER);
    }

    #[test]
    fn invalid_current_restores_valid_current_old() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), NEWER, SlotSource::Downloaded, false);
        break_required_file(&paths.current());
        write_slot(&paths.current_old(), BUNDLED, SlotSource::Downloaded, false);

        let promoted = recover_at_startup(&paths, false).unwrap();
        assert_eq!(promoted, None);
        assert!(!paths.current_old().exists());
        assert_eq!(
            manifest::load(&paths.current()).unwrap().cef_version,
            BUNDLED
        );
        assert!(paths.current().join("libcef.so").is_file());
    }

    #[test]
    fn invalid_current_old_is_dropped_when_current_absent() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current_old(), BUNDLED, SlotSource::Downloaded, false);
        break_required_file(&paths.current_old());

        let promoted = recover_at_startup(&paths, false).unwrap();
        assert_eq!(promoted, None);
        assert!(!paths.current().exists());
        assert!(!paths.current_old().exists());
    }

    #[test]
    fn current_old_file_is_junk_and_does_not_become_current() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        fs::write(paths.current_old(), b"not-a-slot").unwrap();

        recover_at_startup(&paths, false).unwrap();
        assert!(!paths.current().exists());
        assert!(!paths.current_old().exists());
    }

    #[test]
    fn recover_clears_stale_pending_without_candidate() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        set_pending(&paths, NEWER);

        recover_at_startup(&paths, false).unwrap();
        assert_eq!(state::load(&paths).pending_promotion, None);
        assert_eq!(
            manifest::load(&paths.current()).unwrap().cef_version,
            BUNDLED
        );
    }

    #[test]
    fn recover_clears_stale_pending_after_discarding_unverified() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, false);
        set_pending(&paths, NEWER);

        recover_at_startup(&paths, false).unwrap();
        assert!(!paths.candidate().exists());
        assert_eq!(state::load(&paths).pending_promotion, None);
    }

    #[test]
    fn recover_is_idempotent_after_crash_mid_rename() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);
        crash_after_current_to_old(&paths);

        recover_at_startup(&paths, false).unwrap();
        let again = recover_at_startup(&paths, false).unwrap();
        assert_eq!(again, None);
        assert_eq!(manifest::load(&paths.current()).unwrap().cef_version, NEWER);
        assert!(!paths.candidate().exists());
        assert!(!paths.current_old().exists());
        assert_eq!(state::load(&paths).pending_promotion, None);
    }

    #[test]
    fn verified_candidate_missing_files_is_discarded_on_recover() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);
        break_required_file(&paths.candidate());
        set_pending(&paths, NEWER);

        let promoted = recover_at_startup(&paths, false).unwrap();
        assert_eq!(promoted, None);
        assert!(!paths.candidate().exists());
        assert_eq!(
            manifest::load(&paths.current()).unwrap().cef_version,
            BUNDLED
        );
        assert_eq!(state::load(&paths).pending_promotion, None);
    }

    #[test]
    fn promote_does_not_swap_when_verified_candidate_fails_validate() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.current_old(), OLDER, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);
        break_required_file(&paths.candidate());

        let error = promote_candidate(&paths, false).unwrap_err();
        assert!(error.contains("Falta el archivo obligatorio") || error.contains("libcef"));
        assert_eq!(
            manifest::load(&paths.current()).unwrap().cef_version,
            BUNDLED
        );
        assert_eq!(
            manifest::load(&paths.current_old()).unwrap().cef_version,
            OLDER
        );
        assert!(paths.candidate().exists());
    }

    #[test]
    fn corrupt_candidate_manifest_discarded_on_recover() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);
        fs::write(paths.candidate().join("manifest.json"), b"{nope").unwrap();

        let promoted = recover_at_startup(&paths, false).unwrap();
        assert_eq!(promoted, None);
        assert!(!paths.candidate().exists());
    }

    #[test]
    fn empty_candidate_dir_discarded_on_recover() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        fs::create_dir_all(paths.candidate()).unwrap();

        recover_at_startup(&paths, false).unwrap();
        assert!(!paths.candidate().exists());
    }

    #[test]
    fn promote_clears_pending_after_successful_swap() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);
        set_pending(&paths, NEWER);

        let result = promote_candidate(&paths, false).unwrap();
        assert!(matches!(result, PromoteResult::Promoted(_)));
        assert_eq!(state::load(&paths).pending_promotion, None);
        assert!(!manifest::load(&paths.current()).unwrap().verified);
    }

    #[test]
    fn recover_invalid_current_and_broken_candidate_keeps_last_good() {
        let tmp = TempDir::new().unwrap();
        let paths = setup(&tmp);
        write_slot(&paths.current(), NEWER, SlotSource::Downloaded, false);
        break_required_file(&paths.current());
        write_slot(&paths.current_old(), BUNDLED, SlotSource::Downloaded, false);
        write_slot(&paths.candidate(), NEWER, SlotSource::Downloaded, true);
        break_required_file(&paths.candidate());
        set_pending(&paths, NEWER);

        let promoted = recover_at_startup(&paths, false).unwrap();
        assert_eq!(promoted, None);
        assert_eq!(
            manifest::load(&paths.current()).unwrap().cef_version,
            BUNDLED
        );
        assert!(paths.current().join("libcef.so").is_file());
        assert!(!paths.current_old().exists());
        assert!(!paths.candidate().exists());
        assert_eq!(state::load(&paths).pending_promotion, None);
    }
}
