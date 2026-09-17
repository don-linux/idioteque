//! Descarga verificada (tamaño + SHA-1) del tarball CEF.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha1::{Digest, Sha1};

const READ_TIMEOUT: Duration = Duration::from_secs(60);
const COPY_BUF: usize = 64 * 1024;

pub const MIN_FREE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadError {
    Network(String),
    SizeMismatch { expected: u64, actual: u64 },
    Sha1Mismatch { expected: String, actual: String },
    Io(String),
}

impl DownloadError {
    pub fn message(&self) -> String {
        match self {
            DownloadError::Network(error) => {
                format!("No se pudo descargar el runtime CEF: {error}")
            }
            DownloadError::SizeMismatch { expected, actual } => {
                format!("Tamaño incorrecto del archivo CEF: {actual} ≠ {expected}")
            }
            DownloadError::Sha1Mismatch { expected, actual } => {
                format!("SHA1 incorrecto del archivo CEF: {actual} ≠ {expected}")
            }
            DownloadError::Io(error) => format!("No se pudo escribir el archivo CEF: {error}"),
        }
    }
}

/// Temporal de `dest`. `Path::with_extension` sustituye la última extensión:
/// `download.tar.bz2` → `download.tar.part` (el path que usa el updater).
fn part_path(dest: &Path) -> PathBuf {
    dest.with_extension("part")
}

/// Huérfano “intuitivo” `dest` + `.part` (`download.tar.bz2.part`), por si un
/// crash o herramienta externa no usó `with_extension`.
fn stray_part_path(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_os_string();
    name.push(".part");
    PathBuf::from(name)
}

fn remove_part_files(dest: &Path) {
    let _ = fs::remove_file(part_path(dest));
    let _ = fs::remove_file(stray_part_path(dest));
}

/// Descarga `url` a `dest`, hasheando SHA-1 en streaming. Usa `dest` con
/// extensión `.part` como temporal y la borra si algo falla.
pub fn download_verified(
    url: &str,
    dest: &Path,
    expected_size: u64,
    expected_sha1: &str,
) -> Result<(), DownloadError> {
    download_verified_timed(url, dest, expected_size, expected_sha1, READ_TIMEOUT)
}

fn download_verified_timed(
    url: &str,
    dest: &Path,
    expected_size: u64,
    expected_sha1: &str,
    timeout: Duration,
) -> Result<(), DownloadError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            DownloadError::Io(format!("No se pudo crear `{}`: {error}", parent.display()))
        })?;
    }

    let part = part_path(dest);
    remove_part_files(dest);

    let result = (|| {
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|error| DownloadError::Network(error.to_string()))?;
        let mut response = client
            .get(url)
            .send()
            .map_err(|error| DownloadError::Network(error.to_string()))?;
        if !response.status().is_success() {
            return Err(DownloadError::Network(format!(
                "HTTP {}",
                response.status().as_u16()
            )));
        }

        let mut file = File::create(&part).map_err(|error| {
            DownloadError::Io(format!("No se pudo crear `{}`: {error}", part.display()))
        })?;
        let mut hasher = Sha1::new();
        let mut buf = vec![0u8; COPY_BUF];
        let mut actual = 0u64;
        loop {
            let n = response.read(&mut buf).map_err(|error| {
                DownloadError::Network(format!("lectura interrumpida: {error}"))
            })?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|error| {
                DownloadError::Io(format!("No se pudo escribir el temporal: {error}"))
            })?;
            hasher.update(&buf[..n]);
            actual += n as u64;
        }
        file.flush()
            .map_err(|error| DownloadError::Io(error.to_string()))?;
        drop(file);

        if actual != expected_size {
            return Err(DownloadError::SizeMismatch {
                expected: expected_size,
                actual,
            });
        }

        let actual_sha1 = hex::encode(hasher.finalize());
        if !sha1_eq(&actual_sha1, expected_sha1) {
            return Err(DownloadError::Sha1Mismatch {
                expected: expected_sha1.to_string(),
                actual: actual_sha1,
            });
        }

        fs::rename(&part, dest).map_err(|error| {
            DownloadError::Io(format!(
                "No se pudo mover `{}` a `{}`: {error}",
                part.display(),
                dest.display()
            ))
        })?;
        Ok(())
    })();

    if result.is_err() {
        remove_part_files(dest);
    }
    result
}

#[allow(dead_code)]
pub fn sha1_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("No se pudo leer `{}`: {error}", path.display()))?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; COPY_BUF];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|error| format!("No se pudo leer `{}`: {error}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn available_space(dir: &Path) -> Result<u64, String> {
    fs2::available_space(dir).map_err(|error| {
        format!(
            "No se pudo consultar el espacio libre en `{}`: {error}",
            dir.display()
        )
    })
}

/// Contrato §8 / updater: exige `min_bytes` libres en `dir` antes de bajar.
#[allow(dead_code)]
pub fn require_free_space(dir: &Path, min_bytes: u64) -> Result<u64, DownloadError> {
    let free = available_space(dir).map_err(DownloadError::Io)?;
    if free < min_bytes {
        Err(DownloadError::Io(format!(
            "espacio insuficiente: {free} < {min_bytes}"
        )))
    } else {
        Ok(free)
    }
}

/// Atajo del umbral de producto (`MIN_FREE_BYTES` = 2 GiB).
#[allow(dead_code)]
pub fn require_min_free(dir: &Path) -> Result<u64, DownloadError> {
    require_free_space(dir, MIN_FREE_BYTES)
}

fn sha1_eq(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use tempfile::TempDir;

    fn serve_http(status: u16, reason: &str, body: &[u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let body = body.to_vec();
        let reason = reason.to_string();
        thread::spawn(move || {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(_) => return,
            };
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let header = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
        });
        format!("http://{addr}/payload")
    }

    fn serve_body(body: &[u8]) -> String {
        serve_http(200, "OK", body)
    }

    fn serve_truncated(content_length: usize, body: &[u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let body = body.to_vec();
        thread::spawn(move || {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(_) => return,
            };
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n"
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
        });
        format!("http://{addr}/payload")
    }

    fn serve_hang_after_headers() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        thread::spawn(move || {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(_) => return,
            };
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 1048576\r\nConnection: close\r\n\r\n",
            );
            let _ = stream.flush();
            thread::sleep(Duration::from_secs(8));
        });
        format!("http://{addr}/payload")
    }

    fn sha1_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha1::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }

    fn assert_no_dest_or_parts(dest: &Path) {
        assert!(!dest.exists(), "dest leaked {}", dest.display());
        assert!(
            !part_path(dest).exists(),
            "part leaked {}",
            part_path(dest).display()
        );
        assert!(
            !stray_part_path(dest).exists(),
            "stray part leaked {}",
            stray_part_path(dest).display()
        );
    }

    #[test]
    fn download_verified_ok_renames_and_matches_hash() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("download.tar.bz2");
        let body = b"cef-runtime-bytes";
        let url = serve_body(body);
        download_verified(&url, &dest, body.len() as u64, &sha1_hex(body)).expect("ok");
        assert_eq!(fs::read(&dest).unwrap(), body);
        assert!(!part_path(&dest).exists());
        assert_eq!(sha1_file(&dest).unwrap(), sha1_hex(body));
    }

    #[test]
    fn download_verified_wrong_size_cleans_part() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("download.tar.bz2");
        let body = b"short";
        let url = serve_body(body);
        let error = download_verified(&url, &dest, 99, &sha1_hex(body)).unwrap_err();
        match error {
            DownloadError::SizeMismatch {
                expected: 99,
                actual: 5,
            } => {}
            other => panic!("unexpected {other:?}"),
        }
        assert!(error.message().contains("Tamaño incorrecto"));
        assert_no_dest_or_parts(&dest);
    }

    #[test]
    fn download_verified_wrong_sha1_cleans_part() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("file.bin");
        let preexisting = b"keep-me-on-sha1-fail";
        fs::write(&dest, preexisting).unwrap();
        let body = b"abcdefgh";
        let url = serve_body(body);
        let expected = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
        let error = download_verified(&url, &dest, body.len() as u64, expected).unwrap_err();
        match &error {
            DownloadError::Sha1Mismatch {
                expected: exp,
                actual,
            } => {
                assert_eq!(exp, expected);
                assert_eq!(actual, &sha1_hex(body));
                assert_ne!(actual, expected);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(error.message().contains("SHA1 incorrecto"));
        assert_eq!(fs::read(&dest).unwrap(), preexisting);
        assert!(!part_path(&dest).exists());
        assert!(!stray_part_path(&dest).exists());
    }

    #[test]
    fn download_verified_sha1_rejects_empty_and_short_expected() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("download.tar.bz2");
        let body = b"not-empty";
        for expected in ["", "abcd", "deadbeef"] {
            let url = serve_body(body);
            let error = download_verified(&url, &dest, body.len() as u64, expected).unwrap_err();
            match error {
                DownloadError::Sha1Mismatch { expected: exp, .. } => {
                    assert_eq!(exp, expected);
                }
                other => panic!("expected Sha1Mismatch for {expected:?}, got {other:?}"),
            }
            assert_no_dest_or_parts(&dest);
        }
    }

    #[test]
    fn download_verified_sha1_accepts_uppercase_and_trimmed() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("download.tar.bz2");
        let body = b"case-fold";
        let url = serve_body(body);
        let expected = format!("\t{} \n", sha1_hex(body).to_ascii_uppercase());
        download_verified(&url, &dest, body.len() as u64, &expected).expect("case/trim");
        assert_eq!(fs::read(&dest).unwrap(), body);
        assert!(!part_path(&dest).exists());
    }

    #[test]
    fn download_verified_http_error_statuses_are_network_and_clean() {
        let tmp = TempDir::new().unwrap();
        for (status, reason) in [
            (404, "Not Found"),
            (500, "Internal Server Error"),
            (502, "Bad Gateway"),
            (503, "Service Unavailable"),
        ] {
            let dest = tmp.path().join(format!("download-{status}.tar.bz2"));
            let leftover = b"stale-part";
            fs::write(part_path(&dest), leftover).unwrap();
            fs::write(stray_part_path(&dest), leftover).unwrap();
            let url = serve_http(status, reason, b"error-page");
            let error = download_verified(&url, &dest, 99, "deadbeef").unwrap_err();
            match &error {
                DownloadError::Network(msg) => {
                    assert_eq!(msg, &format!("HTTP {status}"), "{error:?}");
                }
                other => panic!("status {status}: unexpected {other:?}"),
            }
            assert!(
                error.message().contains("No se pudo descargar"),
                "{}",
                error.message()
            );
            assert_no_dest_or_parts(&dest);
        }
    }

    #[test]
    fn download_verified_connection_refused_is_network() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("download.tar.bz2");
        fs::write(part_path(&dest), b"orphan").unwrap();
        let error = download_verified(
            "http://127.0.0.1:1/cef.tar.bz2",
            &dest,
            1,
            "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        )
        .unwrap_err();
        match error {
            DownloadError::Network(_) => {}
            other => panic!("unexpected {other:?}"),
        }
        assert!(error.message().contains("No se pudo descargar"));
        assert_no_dest_or_parts(&dest);
    }

    #[test]
    fn download_verified_timeout_after_headers_cleans_part() {
        assert_eq!(READ_TIMEOUT, Duration::from_secs(60));
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("download.tar.bz2");
        fs::write(stray_part_path(&dest), b"old-stray").unwrap();
        let url = serve_hang_after_headers();
        let error = download_verified_timed(
            &url,
            &dest,
            1_048_576,
            "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
            Duration::from_millis(400),
        )
        .unwrap_err();
        match &error {
            DownloadError::Network(msg) => {
                let lower = msg.to_ascii_lowercase();
                assert!(
                    lower.contains("timed out")
                        || lower.contains("timeout")
                        || lower.contains("interrumpida"),
                    "timeout-ish network error, got {msg}"
                );
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_no_dest_or_parts(&dest);
    }

    #[test]
    fn download_verified_truncated_body_is_network_and_cleans() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("download.tar.bz2");
        let body = b"only-four";
        let url = serve_truncated(100, body);
        let error = download_verified(&url, &dest, 100, &sha1_hex(body)).unwrap_err();
        match &error {
            DownloadError::Network(msg) => {
                assert!(
                    msg.contains("interrumpida") || msg.to_ascii_lowercase().contains("body"),
                    "{msg}"
                );
            }
            other => panic!("truncated Content-Length is a body error, got {other:?}"),
        }
        assert!(error.message().contains("No se pudo descargar"));
        assert_no_dest_or_parts(&dest);
    }

    #[test]
    fn download_verified_cleans_orphan_parts_on_failure() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("download.tar.bz2");
        assert_eq!(
            part_path(&dest).file_name().unwrap(),
            "download.tar.part",
            "updater dest download.tar.bz2 uses with_extension"
        );
        assert_eq!(
            stray_part_path(&dest).file_name().unwrap(),
            "download.tar.bz2.part"
        );
        fs::write(part_path(&dest), b"CRASH-ORPHAN-with-extension").unwrap();
        fs::write(stray_part_path(&dest), b"CRASH-ORPHAN-appended").unwrap();
        let preexisting = b"already-downloaded";
        fs::write(&dest, preexisting).unwrap();

        let url = serve_http(404, "Not Found", b"");
        let error = download_verified(&url, &dest, 10, "abcd").unwrap_err();
        match error {
            DownloadError::Network(msg) => assert_eq!(msg, "HTTP 404"),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(fs::read(&dest).unwrap(), preexisting);
        assert!(!part_path(&dest).exists());
        assert!(!stray_part_path(&dest).exists());
    }

    #[test]
    fn download_verified_cleans_orphan_part_before_overwrite_on_sha1_fail() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("download.tar.bz2");
        fs::write(part_path(&dest), b"stale-partial-bytes").unwrap();
        let body = b"fresh-but-wrong-hash";
        let url = serve_body(body);
        let error = download_verified(
            &url,
            &dest,
            body.len() as u64,
            "0000000000000000000000000000000000000000",
        )
        .unwrap_err();
        match error {
            DownloadError::Sha1Mismatch { .. } => {}
            other => panic!("unexpected {other:?}"),
        }
        assert_no_dest_or_parts(&dest);
    }

    #[test]
    fn download_verified_parent_is_file_is_io() {
        let tmp = TempDir::new().unwrap();
        let blocker = tmp.path().join("not-a-dir");
        fs::write(&blocker, b"x").unwrap();
        let dest = blocker.join("download.tar.bz2");
        let error = download_verified("http://127.0.0.1:1/", &dest, 1, "abcd").unwrap_err();
        match error {
            DownloadError::Io(msg) => assert!(msg.contains("No se pudo crear"), "{msg}"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn available_space_on_tempdir() {
        let tmp = TempDir::new().unwrap();
        let free = available_space(tmp.path()).expect("space");
        assert!(free > 0);
        assert_eq!(MIN_FREE_BYTES, 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn available_space_missing_path_errors() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("nope").join("nested");
        let error = available_space(&missing).expect_err("missing");
        assert!(
            error.contains("No se pudo consultar el espacio libre"),
            "{error}"
        );
    }

    #[test]
    fn require_free_space_rejects_below_threshold_and_min_free_bytes() {
        let tmp = TempDir::new().unwrap();
        let free = available_space(tmp.path()).expect("space");
        assert_eq!(require_free_space(tmp.path(), 0).unwrap(), free);
        assert_eq!(require_free_space(tmp.path(), 1).unwrap(), free);
        assert_eq!(require_free_space(tmp.path(), free).unwrap(), free);

        let too_much = free.saturating_add(1);
        let error = require_free_space(tmp.path(), too_much).unwrap_err();
        match &error {
            DownloadError::Io(msg) => {
                assert!(msg.contains("espacio insuficiente"), "{msg}");
                assert!(msg.contains(&free.to_string()), "{msg}");
                assert!(msg.contains(&too_much.to_string()), "{msg}");
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(error.message().contains("No se pudo escribir"));

        let impossible = require_free_space(tmp.path(), u64::MAX).unwrap_err();
        match impossible {
            DownloadError::Io(msg) => {
                assert!(msg.contains("espacio insuficiente"));
                assert!(msg.contains(&u64::MAX.to_string()));
            }
            other => panic!("unexpected {other:?}"),
        }

        match require_min_free(tmp.path()) {
            Ok(reported) => {
                assert_eq!(reported, free);
                assert!(free >= MIN_FREE_BYTES);
            }
            Err(DownloadError::Io(msg)) => {
                assert!(free < MIN_FREE_BYTES);
                assert!(msg.contains("espacio insuficiente"));
                assert!(msg.contains(&MIN_FREE_BYTES.to_string()));
            }
            Err(other) => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn require_min_free_missing_path_is_io() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("absent");
        match require_min_free(&missing) {
            Err(DownloadError::Io(msg)) => {
                assert!(
                    msg.contains("No se pudo consultar el espacio libre"),
                    "{msg}"
                );
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[cfg(target_os = "linux")]
    fn assert_write_fails_and_cleans(dir: &Path, body_len: usize) {
        let dest = dir.join("download.tar.bz2");
        fs::write(part_path(&dest), b"pre-orphan").ok();
        let body = vec![0x61u8; body_len];
        let url = serve_body(&body);
        let error =
            download_verified(&url, &dest, body.len() as u64, &sha1_hex(&body)).unwrap_err();
        match error {
            DownloadError::Io(msg) => {
                assert!(
                    msg.contains("No se pudo escribir el temporal")
                        || msg.contains("No se pudo crear"),
                    "{msg}"
                );
            }
            other => panic!("expected Io (disk full / EFBIG), got {other:?}"),
        }
        assert_no_dest_or_parts(&dest);
    }

    /// ENOSPC en tmpfs (userns) o EFBIG por `RLIMIT_FSIZE`. Mismo cleanup que disco lleno.
    #[cfg(target_os = "linux")]
    #[test]
    fn download_verified_disk_full_cleans_part() {
        match std::env::var("IDQ_DOWNLOAD_DISK_CHILD").ok().as_deref() {
            Some("tmpfs") => {
                let dir = TempDir::new().unwrap();
                let mnt = dir.path().join("mnt");
                fs::create_dir(&mnt).unwrap();
                let c_target = std::ffi::CString::new(mnt.to_str().unwrap()).unwrap();
                let c_tmpfs = std::ffi::CString::new("tmpfs").unwrap();
                let c_opts = std::ffi::CString::new("size=256k").unwrap();
                let rc = unsafe {
                    libc::mount(
                        c_tmpfs.as_ptr(),
                        c_target.as_ptr(),
                        c_tmpfs.as_ptr(),
                        0,
                        c_opts.as_ptr().cast(),
                    )
                };
                if rc != 0 {
                    std::process::exit(90);
                }
                assert_write_fails_and_cleans(&mnt, 400 * 1024);
                return;
            }
            Some("fsize") => {
                let lim = libc::rlimit {
                    rlim_cur: 4096,
                    rlim_max: 4096,
                };
                unsafe {
                    libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
                    assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE, &lim), 0);
                }
                let dir = TempDir::new().unwrap();
                assert_write_fails_and_cleans(dir.path(), 64 * 1024);
                return;
            }
            Some(other) => panic!("IDQ_DOWNLOAD_DISK_CHILD desconocido: {other}"),
            None => {}
        }

        let exe = std::env::current_exe().expect("test exe");
        let name = thread::current()
            .name()
            .expect("test thread name")
            .to_string();

        let tmpfs = std::process::Command::new("unshare")
            .args(["--user", "--map-root-user", "--mount"])
            .arg(&exe)
            .arg("--exact")
            .arg(&name)
            .arg("--nocapture")
            .env("IDQ_DOWNLOAD_DISK_CHILD", "tmpfs")
            .output();

        match tmpfs {
            Ok(out) if out.status.success() => return,
            Ok(out) if out.status.code() == Some(90) => {}
            Ok(out) => {
                panic!(
                    "tmpfs ENOSPC child failed (status {:?}):\nstdout:\n{}\nstderr:\n{}",
                    out.status.code(),
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
            }
            Err(error) => {
                eprintln!("unshare no disponible ({error}); fallback RLIMIT_FSIZE");
            }
        }

        let fsize = std::process::Command::new(&exe)
            .arg("--exact")
            .arg(&name)
            .arg("--nocapture")
            .env("IDQ_DOWNLOAD_DISK_CHILD", "fsize")
            .output()
            .expect("spawn fsize child");
        assert!(
            fsize.status.success(),
            "RLIMIT_FSIZE child failed (status {:?}):\nstdout:\n{}\nstderr:\n{}",
            fsize.status.code(),
            String::from_utf8_lossy(&fsize.stdout),
            String::from_utf8_lossy(&fsize.stderr)
        );
    }
}
