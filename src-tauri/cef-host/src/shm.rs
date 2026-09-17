//! Dónde crea Chromium su memoria compartida (contrato 4.1).
//!
//! Chromium en Linux **quiere** `/dev/shm` (tmpfs). No usa `memfd` ni
//! `shm_open`: `PlatformSharedMemoryRegion` crea un fichero temporal, lo
//! `unlink`ea y hace `mmap` (`platform_shared_memory_region_posix.cc`) para
//! poder honrar el flag. Ahí van TransferBuffers GPU, Mojo y fuentes.
//!
//! Niveles oficiales (solo dos, `base_switches.h` `kDisableDevShmUsage`,
//! crbug/715363):
//!
//! 1. Default: `/dev/shm` usable → cero flags (`GetShmemTempDir` → `/dev/shm`).
//! 2. Único workaround documentado: `--disable-dev-shm-usage` → `$TMPDIR`/`/tmp`.
//!
//! ChromeDriver solo hace `access(W_OK|X_OK)`. Chromium **no** autodetecta
//! tamaño. Docker default `--shm-size` = 64 MiB es **un** caso de fallo, no
//! la política de producto (deb/rpm/AppImage en escritorio con 1–8 GiB
//! siguen en `DevShm`).
//!
//! Extras de idioteque (no están en CEF/Chromium). Se conservan porque
//! quitarlos tumba el arranque (política de workarounds / ADVERSARIAL.md):
//!
//! - Probe `fallocate` de [`PROBE_BYTES`] (128 MiB): un check tipo ChromeDriver
//!   acepta un `/dev/shm` de 64 MiB y Chromium muere segundos después
//!   (`ENOSPC`, `TransferBuffer::Initialize() failed`).
//! - [`ShmPolicy::CacheDir`]: si el temp oficial también falla (`EDQUOT` en
//!   tmpfs con `usrquota`, systemd ≥ 258, no es un quirk de Ubuntu) se pone
//!   `TMPDIR=<cache>/shm` en disco. Sin esto el flag oficial apunta al mismo
//!   `$TMPDIR` roto.
//!
//! No se copia `libcef` ni el slot a `/dev/shm`. El flag no se ata al sandbox.

use std::io;
use std::path::{Path, PathBuf};

/// Lo que reserva el probe. El `/dev/shm` de Docker mide exactamente
/// 64 MiB: un probe de 64 MiB lo pasaría recién montado y Chromium se
/// quedaría sin sitio a los pocos segundos.
pub const PROBE_BYTES: u64 = 128 * 1024 * 1024;

/// Default `--shm-size` de Docker/dockerd cuando se omite. No es umbral de
/// producto: la decisión es el probe, no `size == 64 MiB`.
#[cfg(test)]
pub const DOCKER_DEFAULT_SHM_BYTES: u64 = 64 * 1024 * 1024;

/// Chromium `kDisableDevShmUsage` (sin `--`).
pub const DISABLE_DEV_SHM_USAGE: &str = "disable-dev-shm-usage";

/// Los dos niveles que documenta Chromium. `CacheDir` es el extra de
/// idioteque y se proyecta al segundo.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficialShmLevel {
    /// Default Chrome: ficheros anónimos en `/dev/shm`.
    DevShm,
    /// `--disable-dev-shm-usage` → `GetTempDir()` (`$TMPDIR` / `/tmp`).
    DisableDevShmUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShmPolicy {
    /// `/dev/shm` sirve: sin switch, lo mismo que Chrome.
    DevShm,
    /// `/dev/shm` no sirve pero el temp dir sí: `--disable-dev-shm-usage`.
    TempDir(PathBuf),
    /// Ninguno sirve: `--disable-dev-shm-usage` y `TMPDIR=<cache>/shm`
    /// (en disco, sin cuota; los ficheros se borran al crearse).
    CacheDir(PathBuf),
}

impl ShmPolicy {
    pub fn disable_dev_shm(&self) -> bool {
        !matches!(self, ShmPolicy::DevShm)
    }

    pub fn tmpdir_override(&self) -> Option<&Path> {
        match self {
            ShmPolicy::CacheDir(dir) => Some(dir),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn official_level(&self) -> OfficialShmLevel {
        if self.disable_dev_shm() {
            OfficialShmLevel::DisableDevShmUsage
        } else {
            OfficialShmLevel::DevShm
        }
    }
}

pub fn cache_shm_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("shm")
}

pub fn choose(dev_shm_ok: bool, temp_dir: &Path, temp_ok: bool, cache_dir: &Path) -> ShmPolicy {
    if dev_shm_ok {
        ShmPolicy::DevShm
    } else if temp_ok {
        ShmPolicy::TempDir(temp_dir.to_path_buf())
    } else {
        ShmPolicy::CacheDir(cache_shm_dir(cache_dir))
    }
}

/// Decide la política para este arranque. Solo Linux conoce el switch.
pub fn decide(cache_dir: &Path) -> ShmPolicy {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = cache_dir;
        ShmPolicy::DevShm
    }
    #[cfg(target_os = "linux")]
    {
        decide_from(
            Path::new("/dev/shm"),
            &std::env::temp_dir(),
            cache_dir,
            PROBE_BYTES,
        )
    }
}

/// Misma política que [`decide`], con directorios y tamaño inyectables.
/// Sirve para 64 MiB vs shm grande, permisos y quota sin asumir Ubuntu.
pub fn decide_from(
    dev_shm: &Path,
    temp_dir: &Path,
    cache_dir: &Path,
    probe_bytes: u64,
) -> ShmPolicy {
    let dev_shm_ok = match probe_dir(dev_shm, probe_bytes) {
        Ok(()) => true,
        Err(error) => {
            eprintln!(
                "cef-host: {} no sirve para memoria compartida: {error}",
                dev_shm.display()
            );
            false
        }
    };
    let temp_ok = if dev_shm_ok {
        false
    } else {
        match probe_dir(temp_dir, probe_bytes) {
            Ok(()) => true,
            Err(error) => {
                eprintln!(
                    "cef-host: {} no sirve para memoria compartida: {error}",
                    temp_dir.display()
                );
                false
            }
        }
    };
    choose(dev_shm_ok, temp_dir, temp_ok, cache_dir)
}

/// `TMPDIR` para el caso `CacheDir`; los subprocesos lo heredan.
/// Los niveles oficiales no tocan el env: Chromium usa `GetTempDir()`.
pub fn apply_env(policy: &ShmPolicy) -> io::Result<()> {
    if let Some(dir) = policy.tmpdir_override() {
        std::fs::create_dir_all(dir)?;
        std::env::set_var("TMPDIR", dir);
    }
    Ok(())
}

/// Crea un fichero, lo borra y reserva `bytes` en él: detecta a la vez
/// permisos, tamaño del tmpfs y cuota por usuario.
pub fn probe_dir(dir: &Path, bytes: u64) -> io::Result<()> {
    probe_dir_with(dir, bytes, reserve)
}

/// ChromeDriver `EnsureSharedMemory()`: solo `access(W_OK|X_OK)`, sin
/// reservar. Un `/dev/shm` de 64 MiB escribible pasa este check.
#[cfg(all(test, unix))]
pub fn dir_accessible_wx(dir: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(dir.as_os_str().as_bytes()) else {
        return false;
    };
    unsafe { libc::access(c.as_ptr(), libc::W_OK | libc::X_OK) == 0 }
}

/// `ENOSPC` / `EDQUOT`: el directorio no puede respaldar la reserva.
/// `statvfs`/`df` pueden mentir con `usrquota`; el probe no.
#[cfg(test)]
pub fn is_shm_capacity_error(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(libc::ENOSPC) | Some(libc::EDQUOT)
    )
}

#[cfg(test)]
pub fn is_shm_permission_error(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(libc::EACCES) | Some(libc::EPERM) | Some(libc::EROFS)
    )
}

pub fn extra_switch_name(raw: &str) -> &str {
    let s = raw.trim();
    let s = s.strip_prefix("--").unwrap_or(s);
    s.split_once('=').map(|(k, _)| k).unwrap_or(s)
}

pub fn extra_forces_disable_dev_shm(extra_switches: &[String]) -> bool {
    extra_switches
        .iter()
        .any(|raw| extra_switch_name(raw) == DISABLE_DEV_SHM_USAGE)
}

/// El bloque shm de `app.rs`: flag oficial si la política lo pide **o**
/// `IDIOTEQUE_CEF_ARGS` lo fuerza (aunque el probe haya elegido `DevShm`).
pub fn command_line_disables_dev_shm(policy_disable: bool, extra_switches: &[String]) -> bool {
    policy_disable || extra_forces_disable_dev_shm(extra_switches)
}

fn probe_dir_with<F>(dir: &Path, bytes: u64, reserve_fn: F) -> io::Result<()>
where
    F: FnOnce(&std::fs::File, u64) -> io::Result<()>,
{
    let name = format!(
        ".idq-shm-probe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let path = dir.join(name);
    let file = create_probe_file(&path)?;
    let unlinked = std::fs::remove_file(&path);
    let reserved = reserve_fn(&file, bytes);
    drop(file);
    if unlinked.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    reserved
}

fn create_probe_file(path: &Path) -> io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(target_os = "linux")]
fn reserve(file: &std::fs::File, bytes: u64) -> io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let len = libc::off_t::try_from(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "probe demasiado grande"))?;
    let ret = unsafe { libc::fallocate(file.as_raw_fd(), 0, 0, len) };
    if ret == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::EOPNOTSUPP) | Some(libc::ENOSYS) => write_zeros(file, bytes),
        _ => Err(error),
    }
}

#[cfg(not(target_os = "linux"))]
fn reserve(file: &std::fs::File, bytes: u64) -> io::Result<()> {
    write_zeros(file, bytes)
}

fn write_zeros(file: &std::fs::File, bytes: u64) -> io::Result<()> {
    use std::io::Write;
    let chunk = vec![0u8; 1024 * 1024];
    let mut left = bytes;
    let mut file = file;
    while left > 0 {
        let n = chunk.len().min(usize::try_from(left).unwrap_or(usize::MAX));
        file.write_all(&chunk[..n])?;
        left -= n as u64;
    }
    file.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn scratch_root() -> PathBuf {
        // Never `std::env::temp_dir()`: `apply_env` may point TMPDIR at a
        // CacheDir that another test deletes (POSIX `/tmp`, not Ubuntu-only).
        #[cfg(unix)]
        {
            PathBuf::from("/tmp")
        }
        #[cfg(not(unix))]
        {
            std::env::temp_dir()
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = scratch_root().join(format!(
            "idq-shm-test-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    fn with_env_lock<T>(f: impl FnOnce() -> T) -> T {
        let _g = ENV_LOCK.lock().expect("env lock");
        let old = std::env::var_os("TMPDIR");
        let result = f();
        match old {
            Some(v) => std::env::set_var("TMPDIR", v),
            None => std::env::remove_var("TMPDIR"),
        }
        result
    }

    fn leftovers(dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    #[cfg(target_os = "linux")]
    struct SizedTmpfs {
        dir: PathBuf,
        mounted: bool,
    }

    #[cfg(target_os = "linux")]
    impl Drop for SizedTmpfs {
        fn drop(&mut self) {
            if self.mounted {
                let _ = std::process::Command::new("sudo")
                    .args(["-n", "umount", "-l"])
                    .arg(&self.dir)
                    .status();
            }
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// tmpfs de tamaño fijo: analogía de `/dev/shm` en Docker (64 MiB) o
    /// un desktop con shm holgado. No es un default de Ubuntu.
    #[cfg(target_os = "linux")]
    fn mount_tmpfs(tag: &str, size: &str) -> SizedTmpfs {
        let dir = temp_dir(tag);
        let data = format!("size={size},mode=1777");
        let status = std::process::Command::new("sudo")
            .args(["-n", "mount", "-t", "tmpfs", "-o", &data, "tmpfs"])
            .arg(&dir)
            .status()
            .expect("spawn sudo mount");
        if status.success() {
            return SizedTmpfs { dir, mounted: true };
        }
        let target = std::ffi::CString::new(dir.to_string_lossy().as_bytes()).unwrap();
        let fstype = std::ffi::CString::new("tmpfs").unwrap();
        let cdata = std::ffi::CString::new(data).unwrap();
        let rc = unsafe {
            libc::mount(
                fstype.as_ptr(),
                target.as_ptr(),
                fstype.as_ptr(),
                libc::MS_NOSUID | libc::MS_NODEV,
                cdata.as_ptr().cast(),
            )
        };
        assert_eq!(
            rc,
            0,
            "no se pudo montar tmpfs size={size} en {} (sudo -n mount o CAP_SYS_ADMIN): {}",
            dir.display(),
            io::Error::last_os_error()
        );
        SizedTmpfs { dir, mounted: true }
    }

    #[test]
    fn choose_prefers_dev_shm_then_temp_then_cache() {
        let temp = Path::new("/tmp");
        let cache = Path::new("/home/x/.idioteque/cef/profile");
        assert_eq!(choose(true, temp, true, cache), ShmPolicy::DevShm);
        assert_eq!(choose(true, temp, false, cache), ShmPolicy::DevShm);
        assert_eq!(
            choose(false, temp, true, cache),
            ShmPolicy::TempDir(temp.to_path_buf())
        );
        assert_eq!(
            choose(false, temp, false, cache),
            ShmPolicy::CacheDir(cache.join("shm"))
        );
    }

    #[test]
    fn official_levels_are_only_two() {
        assert_eq!(ShmPolicy::DevShm.official_level(), OfficialShmLevel::DevShm);
        assert_eq!(
            ShmPolicy::TempDir("/var/tmp".into()).official_level(),
            OfficialShmLevel::DisableDevShmUsage
        );
        assert_eq!(
            ShmPolicy::CacheDir("/c/shm".into()).official_level(),
            OfficialShmLevel::DisableDevShmUsage
        );
    }

    #[test]
    fn only_dev_shm_keeps_the_switch_off() {
        assert!(!ShmPolicy::DevShm.disable_dev_shm());
        assert!(ShmPolicy::TempDir("/tmp".into()).disable_dev_shm());
        assert!(ShmPolicy::CacheDir("/c/shm".into()).disable_dev_shm());
        assert_eq!(ShmPolicy::DevShm.tmpdir_override(), None);
        assert_eq!(ShmPolicy::TempDir("/tmp".into()).tmpdir_override(), None);
        assert_eq!(
            ShmPolicy::CacheDir("/c/shm".into()).tmpdir_override(),
            Some(Path::new("/c/shm"))
        );
    }

    #[test]
    fn cache_dir_is_tmpdir_for_anonymous_files_not_libcef() {
        let cache = Path::new("/opt/idioteque/cef/profile");
        let policy = choose(false, Path::new("/tmp"), false, cache);
        match policy {
            ShmPolicy::CacheDir(dir) => {
                assert_eq!(dir, cache.join("shm"));
                assert!(!dir.starts_with("/dev/shm"));
                assert_ne!(dir, Path::new("/dev/shm"));
            }
            other => panic!("expected CacheDir, got {other:?}"),
        }
    }

    #[test]
    fn probe_bytes_is_stricter_than_docker_default_not_a_product_size() {
        assert!(PROBE_BYTES > DOCKER_DEFAULT_SHM_BYTES);
        assert_eq!(PROBE_BYTES, 128 * 1024 * 1024);
        // Escritorio 1 GiB / 8 GiB: el probe cabe; no se fuerza el flag.
        assert!(1024 * 1024 * 1024 > PROBE_BYTES);
        assert!(8 * 1024 * 1024 * 1024 > PROBE_BYTES);
    }

    #[test]
    fn sixty_four_mib_is_not_hardcoded_disable() {
        // Un dir escribible de 64 MiB no desactiva shm por ser 64.
        // Solo el probe (reserva real) decide.
        let cache = Path::new("/cache");
        assert_eq!(
            choose(true, Path::new("/tmp"), false, cache),
            ShmPolicy::DevShm
        );
        assert!(!command_line_disables_dev_shm(false, &[]));
    }

    #[test]
    fn probe_succeeds_and_leaves_no_residue() {
        let dir = temp_dir("ok");
        probe_dir(&dir, 1024 * 1024).expect("probe");
        assert!(
            leftovers(&dir).is_empty(),
            "el probe dejó ficheros: {:?}",
            leftovers(&dir)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn probe_fails_on_missing_dir() {
        let dir = temp_dir("missing").join("no-existe");
        let err = probe_dir(&dir, 1024).expect_err("missing");
        assert!(!is_shm_capacity_error(&err));
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn probe_fails_when_path_is_a_file() {
        let dir = temp_dir("file");
        let file = dir.join("not-a-dir");
        std::fs::write(&file, b"x").unwrap();
        assert!(probe_dir(&file, 1024).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn probe_fails_on_unwritable_dir() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let dir = temp_dir("ro");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        assert!(
            !dir_accessible_wx(&dir),
            "ChromeDriver access(W_OK|X_OK) must fail on 0500"
        );
        let result = probe_dir(&dir, 1024);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let err = result.expect_err("0o500");
        assert!(
            is_shm_permission_error(&err)
                || err.kind() == io::ErrorKind::PermissionDenied
                || err.raw_os_error() == Some(libc::EACCES),
            "{err:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn probe_fails_on_world_sticky_without_owner_write() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        // 01555: sticky + r-x, sin write. No es el 1777 de /dev/shm.
        let dir = temp_dir("sticky-ro");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o1555)).unwrap();
        let result = probe_dir(&dir, 1024);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            result.is_err(),
            "01555 must fail; 01755 would still be owner-writable"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn probe_fails_on_root_owned_0700_dir() {
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let dir = temp_dir("root-owned");
        let chown = std::process::Command::new("sudo")
            .args(["-n", "chown", "root:root"])
            .arg(&dir)
            .status();
        let chmod = std::process::Command::new("sudo")
            .args(["-n", "chmod", "0700"])
            .arg(&dir)
            .status();
        match (chown, chmod) {
            (Ok(a), Ok(b)) if a.success() && b.success() => {}
            _ => {
                let _ = std::fs::remove_dir_all(&dir);
                panic!("sudo -n chown/chmod root 0700 is required for this permission case");
            }
        }
        assert!(
            !dir_accessible_wx(&dir),
            "ChromeDriver would also refuse a root-only /dev/shm"
        );
        let result = probe_dir(&dir, 1024);
        let _ = std::process::Command::new("sudo")
            .args(["-n", "rm", "-rf"])
            .arg(&dir)
            .status();
        assert!(result.is_err(), "root-owned 0700 must fail for uid!=0");
    }

    #[cfg(unix)]
    #[test]
    fn probe_accepts_owner_writable_0700_without_requiring_1777() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("owner-700");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        probe_dir(&dir, 64 * 1024).expect("0700 owned dir is enough (not Ubuntu 1777-only)");
        assert!(dir_accessible_wx(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn probe_accepts_typical_dev_shm_1777() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("sticky-1777");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o1777)).unwrap();
        probe_dir(&dir, 64 * 1024).expect("1777");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_zeros_fallback_reserves_exact_size() {
        let dir = temp_dir("zeros");
        let path = dir.join("f");
        let file = create_probe_file(&path).unwrap();
        write_zeros(&file, 3 * 1024 * 1024 + 17).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().len(),
            3 * 1024 * 1024 + 17
        );
        drop(file);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_env_creates_dir_only_for_cache_policy() {
        with_env_lock(|| {
            let dir = temp_dir("env").join("shm");
            assert!(!dir.exists());
            apply_env(&ShmPolicy::CacheDir(dir.clone())).unwrap();
            assert!(dir.is_dir());
            assert_eq!(std::env::var_os("TMPDIR").as_deref(), Some(dir.as_os_str()));
            let _ = std::fs::remove_dir_all(dir.parent().unwrap());
        });
    }

    #[test]
    fn apply_env_does_not_touch_tmpdir_on_official_levels() {
        with_env_lock(|| {
            let marker = temp_dir("keep-tmpdir");
            std::env::set_var("TMPDIR", &marker);
            apply_env(&ShmPolicy::DevShm).unwrap();
            apply_env(&ShmPolicy::TempDir(marker.join("unused").into())).unwrap();
            assert_eq!(
                std::env::var_os("TMPDIR").as_deref(),
                Some(marker.as_os_str())
            );
            assert!(!marker.join("unused").exists());
            let _ = std::fs::remove_dir_all(&marker);
        });
    }

    #[test]
    fn apply_env_errors_when_cache_parent_is_a_file() {
        with_env_lock(|| {
            let dir = temp_dir("env-file");
            let file = dir.join("notdir");
            std::fs::write(&file, b"x").unwrap();
            let err = apply_env(&ShmPolicy::CacheDir(file.join("shm"))).expect_err("mkdir");
            assert!(err.kind() == io::ErrorKind::AlreadyExists || err.raw_os_error().is_some());
            let _ = std::fs::remove_dir_all(&dir);
        });
    }

    #[test]
    fn capacity_errors_include_enospc_and_edquot() {
        let enospc = io::Error::from_raw_os_error(libc::ENOSPC);
        let edquot = io::Error::from_raw_os_error(libc::EDQUOT);
        let eacces = io::Error::from_raw_os_error(libc::EACCES);
        assert!(is_shm_capacity_error(&enospc));
        assert!(is_shm_capacity_error(&edquot));
        assert!(!is_shm_capacity_error(&eacces));
        assert!(is_shm_permission_error(&eacces));
        assert!(!is_shm_permission_error(&enospc));
        assert!(!is_shm_permission_error(&edquot));
    }

    #[test]
    fn probe_surfaces_injected_edquot_and_leaves_no_residue() {
        fn boom(_: &std::fs::File, _: u64) -> io::Result<()> {
            Err(io::Error::from_raw_os_error(libc::EDQUOT))
        }
        let dir = temp_dir("edquot");
        let err = probe_dir_with(&dir, 4096, boom).expect_err("edquot");
        assert_eq!(err.raw_os_error(), Some(libc::EDQUOT));
        assert!(is_shm_capacity_error(&err));
        assert!(leftovers(&dir).is_empty(), "{:?}", leftovers(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn edquot_and_enospc_both_leave_dev_shm_and_take_official_flag() {
        let temp = Path::new("/var/tmp");
        let cache = Path::new("/cache");
        // Cualquier fallo del probe (ENOSPC, EDQUOT, EACCES) es "no sirve".
        let from_enospc = choose(false, temp, true, cache);
        let from_edquot = choose(false, temp, true, cache);
        assert_eq!(from_enospc, from_edquot);
        assert_eq!(
            from_enospc.official_level(),
            OfficialShmLevel::DisableDevShmUsage
        );
        assert!(from_enospc.disable_dev_shm());
    }

    #[test]
    fn official_two_levels_cannot_move_tmpdir_when_temp_probe_fails() {
        // Sin CacheDir, el flag oficial sigue usando GetTempDir() — el mismo
        // temp que acaba de fallar. El arranque de Chromium rompería igual.
        let temp = Path::new("/tmp-quota-full");
        let cache = Path::new("/home/x/.idioteque/cef/profile");
        let policy = choose(false, temp, false, cache);
        assert_eq!(
            policy.official_level(),
            OfficialShmLevel::DisableDevShmUsage
        );
        assert_eq!(
            policy.tmpdir_override(),
            Some(cache.join("shm").as_path()),
            "CacheDir se conserva: el nivel oficial 2 no basta si $TMPDIR tampoco sirve"
        );
    }

    #[test]
    fn extra_args_force_official_flag_even_when_probe_chose_dev_shm() {
        let healthy = ShmPolicy::DevShm;
        assert!(!healthy.disable_dev_shm());
        assert!(!command_line_disables_dev_shm(
            healthy.disable_dev_shm(),
            &[]
        ));

        let forced = crate::args::split_extra_args("--disable-dev-shm-usage");
        assert!(extra_forces_disable_dev_shm(&forced));
        assert!(command_line_disables_dev_shm(
            healthy.disable_dev_shm(),
            &forced
        ));

        let mixed = crate::args::split_extra_args("--disable-gpu --disable-dev-shm-usage --no-sandbox");
        assert!(command_line_disables_dev_shm(false, &mixed));

        let bare = crate::args::split_extra_args("disable-dev-shm-usage");
        assert!(command_line_disables_dev_shm(false, &bare));

        let equals = crate::args::split_extra_args("--disable-dev-shm-usage=1");
        assert!(command_line_disables_dev_shm(false, &equals));
    }

    #[test]
    fn extra_args_without_the_flag_do_not_disable_healthy_shm() {
        let extras = crate::args::split_extra_args("--disable-gpu --use-gl=angle");
        assert!(!extra_forces_disable_dev_shm(&extras));
        assert!(!command_line_disables_dev_shm(false, &extras));
        assert!(!command_line_disables_dev_shm(
            false,
            &crate::args::split_extra_args("")
        ));
    }

    #[test]
    fn shm_switch_is_not_tied_to_sandbox() {
        // no_sandbox no entra en la decisión. TempDir pone el flag; DevShm no.
        assert!(!command_line_disables_dev_shm(
            ShmPolicy::DevShm.disable_dev_shm(),
            &crate::args::split_extra_args("--no-sandbox")
        ));
        assert!(command_line_disables_dev_shm(
            ShmPolicy::TempDir("/tmp".into()).disable_dev_shm(),
            &[]
        ));
    }

    #[test]
    fn decide_follows_live_dev_shm_probe_not_a_cloud_constant() {
        let cache = temp_dir("live-decide");
        let policy = decide(&cache);
        let live_ok = probe_dir(Path::new("/dev/shm"), PROBE_BYTES).is_ok();
        if live_ok {
            assert_eq!(policy, ShmPolicy::DevShm);
            assert_eq!(policy.official_level(), OfficialShmLevel::DevShm);
            assert!(!command_line_disables_dev_shm(
                policy.disable_dev_shm(),
                &[]
            ));
        } else {
            assert_ne!(policy, ShmPolicy::DevShm);
            assert_eq!(
                policy.official_level(),
                OfficialShmLevel::DisableDevShmUsage
            );
            assert!(command_line_disables_dev_shm(policy.disable_dev_shm(), &[]));
        }
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn decide_is_dev_shm_off_linux() {
        assert_eq!(decide(Path::new("/cache")), ShmPolicy::DevShm);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn decide_from_skips_temp_when_dev_shm_holds_probe() {
        let shm = temp_dir("skip-temp-shm");
        let missing_temp = shm.join("no-such-temp");
        let cache = temp_dir("skip-temp-cache");
        let policy = decide_from(&shm, &missing_temp, &cache, 1024 * 1024);
        assert_eq!(policy, ShmPolicy::DevShm);
        let _ = std::fs::remove_dir_all(&shm);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn chrome_driver_access_only_passes_64mib_shm_probe_does_not() {
        let small = mount_tmpfs("cd-64", "64M");
        assert!(
            dir_accessible_wx(&small.dir),
            "ChromeDriver EnsureSharedMemory aceptaría este 64 MiB"
        );
        let err = probe_dir(&small.dir, PROBE_BYTES).expect_err("128MiB on 64MiB");
        assert!(
            is_shm_capacity_error(&err),
            "esperado ENOSPC/EDQUOT, fue {err:?}"
        );
        assert_eq!(err.raw_os_error(), Some(libc::ENOSPC));
        let probes: Vec<_> = leftovers(&small.dir)
            .into_iter()
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(".idq-shm-probe-"))
            })
            .collect();
        assert!(probes.is_empty(), "probe residue: {probes:?}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn decide_from_64mib_shm_takes_official_flag_large_shm_does_not() {
        let small = mount_tmpfs("pol-64", "64M");
        let large = mount_tmpfs("pol-256", "256M");
        let temp_ok = temp_dir("pol-temp");
        let cache = temp_dir("pol-cache");

        let from_small = decide_from(&small.dir, &temp_ok, &cache, PROBE_BYTES);
        assert_eq!(from_small, ShmPolicy::TempDir(temp_ok.clone()));
        assert_eq!(
            from_small.official_level(),
            OfficialShmLevel::DisableDevShmUsage
        );
        assert!(command_line_disables_dev_shm(
            from_small.disable_dev_shm(),
            &[]
        ));

        let from_large = decide_from(&large.dir, &temp_ok, &cache, PROBE_BYTES);
        assert_eq!(from_large, ShmPolicy::DevShm);
        assert_eq!(from_large.official_level(), OfficialShmLevel::DevShm);
        assert!(!command_line_disables_dev_shm(
            from_large.disable_dev_shm(),
            &[]
        ));

        // Un tmpfs de 64 MiB *en otro sitio* no cambia la política del shm grande.
        let from_large_with_small_temp = decide_from(&large.dir, &small.dir, &cache, PROBE_BYTES);
        assert_eq!(from_large_with_small_temp, ShmPolicy::DevShm);

        let _ = std::fs::remove_dir_all(&temp_ok);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn decide_from_cache_dir_when_both_tmpfs_are_too_small() {
        let small = mount_tmpfs("both-64", "64M");
        let smaller = mount_tmpfs("both-32", "32M");
        let cache = temp_dir("both-cache");
        let policy = decide_from(&small.dir, &smaller.dir, &cache, PROBE_BYTES);
        assert_eq!(policy, ShmPolicy::CacheDir(cache.join("shm")));
        assert_eq!(
            policy.official_level(),
            OfficialShmLevel::DisableDevShmUsage
        );
        assert_eq!(policy.tmpdir_override(), Some(cache.join("shm").as_path()));
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn decide_from_unwritable_shm_uses_temp_like_permission_denied() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let shm = temp_dir("perm-shm");
        let temp = temp_dir("perm-temp");
        let cache = temp_dir("perm-cache");
        std::fs::set_permissions(&shm, std::fs::Permissions::from_mode(0o000)).unwrap();
        let policy = decide_from(&shm, &temp, &cache, 64 * 1024);
        std::fs::set_permissions(&shm, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(policy, ShmPolicy::TempDir(temp.clone()));
        let _ = std::fs::remove_dir_all(&shm);
        let _ = std::fs::remove_dir_all(&temp);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn probe_reports_enospc_when_tmpfs_is_already_full() {
        let tiny = mount_tmpfs("full-8", "8M");
        let fill = tiny.dir.join("fill");
        let file = create_probe_file(&fill).unwrap();
        reserve(&file, 7 * 1024 * 1024).expect("fill 7MiB of 8MiB");
        drop(file);
        let err = probe_dir(&tiny.dir, 2 * 1024 * 1024).expect_err("full");
        assert!(
            is_shm_capacity_error(&err),
            "esperado ENOSPC/EDQUOT con tmpfs lleno, fue {err:?}"
        );
        assert_eq!(err.raw_os_error(), Some(libc::ENOSPC));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn enospc_from_fallocate_is_not_retried_as_write_zeros() {
        let small = mount_tmpfs("no-zeros", "16M");
        let path = small.dir.join("direct");
        let file = create_probe_file(&path).unwrap();
        let err = reserve(&file, 64 * 1024 * 1024).expect_err("64 on 16");
        drop(file);
        assert_eq!(err.raw_os_error(), Some(libc::ENOSPC));
        // write_zeros habría escrito hasta llenar y tardado; fallocate corta.
        assert!(std::fs::metadata(&path).unwrap().len() < 16 * 1024 * 1024);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn forced_args_still_apply_when_large_shm_is_healthy() {
        let large = mount_tmpfs("force-256", "256M");
        let temp = temp_dir("force-temp");
        let cache = temp_dir("force-cache");
        let policy = decide_from(&large.dir, &temp, &cache, PROBE_BYTES);
        assert_eq!(policy, ShmPolicy::DevShm);
        let extras = crate::args::split_extra_args("--disable-dev-shm-usage");
        assert!(command_line_disables_dev_shm(
            policy.disable_dev_shm(),
            &extras
        ));
        let _ = std::fs::remove_dir_all(&temp);
        let _ = std::fs::remove_dir_all(&cache);
    }
}
