//! Dónde crea Chromium su memoria compartida (contrato 4.1).
//!
//! Chromium no usa `memfd`: cada región es un fichero en `/dev/shm` que
//! borra al crearlo, o en el temp dir si lleva `--disable-dev-shm-usage`.
//! Ese switch es un parche para contenedores con `/dev/shm` de 64 MiB y no
//! tiene relación con el sandbox. Un tmpfs con `usrquota` (systemd ≥ 258)
//! devuelve `EDQUOT` aunque `df` diga que sobra sitio, así que el probe
//! reserva bytes de verdad en vez de mirar `statvfs`.

use std::io;
use std::path::{Path, PathBuf};

/// Lo que reserva el probe. El `/dev/shm` de Docker mide exactamente
/// 64 MiB: un probe de 64 MiB lo pasaría recién montado y Chromium se
/// quedaría sin sitio a los pocos segundos.
pub const PROBE_BYTES: u64 = 128 * 1024 * 1024;

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
        let dev_shm = Path::new("/dev/shm");
        let dev_shm_ok = match probe_dir(dev_shm, PROBE_BYTES) {
            Ok(()) => true,
            Err(error) => {
                eprintln!("cef-host: /dev/shm no sirve para memoria compartida: {error}");
                false
            }
        };
        let temp_dir = std::env::temp_dir();
        let temp_ok = if dev_shm_ok {
            false
        } else {
            match probe_dir(&temp_dir, PROBE_BYTES) {
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
        choose(dev_shm_ok, &temp_dir, temp_ok, cache_dir)
    }
}

/// `TMPDIR` para el caso `CacheDir`; los subprocesos lo heredan.
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
    let reserved = reserve(&file, bytes);
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

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
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
    fn probe_succeeds_and_leaves_no_residue() {
        let dir = temp_dir("ok");
        probe_dir(&dir, 1024 * 1024).expect("probe");
        let leftovers: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert!(leftovers.is_empty(), "el probe dejó ficheros: {leftovers:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn probe_fails_on_missing_dir() {
        let dir = temp_dir("missing").join("no-existe");
        assert!(probe_dir(&dir, 1024).is_err());
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn probe_fails_on_unwritable_dir() {
        use std::os::unix::fs::PermissionsExt;
        // root ignora los permisos; en ese caso el test no prueba nada.
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let dir = temp_dir("ro");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        let result = probe_dir(&dir, 1024);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_err());
    }

    #[test]
    fn write_zeros_fallback_reserves_exact_size() {
        let dir = temp_dir("zeros");
        let path = dir.join("f");
        let file = create_probe_file(&path).unwrap();
        write_zeros(&file, 3 * 1024 * 1024 + 17).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 3 * 1024 * 1024 + 17);
        drop(file);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_env_creates_dir_only_for_cache_policy() {
        let dir = temp_dir("env").join("shm");
        assert!(!dir.exists());
        apply_env(&ShmPolicy::CacheDir(dir.clone())).unwrap();
        assert!(dir.is_dir());
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}
