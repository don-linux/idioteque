//! Descarga verificada (tamaño + SHA-1) del tarball CEF.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
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

/// Descarga `url` a `dest`, hasheando SHA-1 en streaming. Usa `dest` con
/// extensión `.part` como temporal y la borra si algo falla.
pub fn download_verified(
    url: &str,
    dest: &Path,
    expected_size: u64,
    expected_sha1: &str,
) -> Result<(), DownloadError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            DownloadError::Io(format!("No se pudo crear `{}`: {error}", parent.display()))
        })?;
    }

    let part = dest.with_extension("part");
    let _ = fs::remove_file(&part);

    let result = (|| {
        let client = reqwest::blocking::Client::builder()
            .timeout(READ_TIMEOUT)
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
        let _ = fs::remove_file(&part);
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

    fn serve_body(body: &[u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let body = body.to_vec();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
        });
        format!("http://{addr}/payload")
    }

    fn sha1_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha1::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }

    #[test]
    fn download_verified_ok_renames_and_matches_hash() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("download.tar.bz2");
        let body = b"cef-runtime-bytes";
        let url = serve_body(body);
        download_verified(&url, &dest, body.len() as u64, &sha1_hex(body)).expect("ok");
        assert_eq!(fs::read(&dest).unwrap(), body);
        assert!(!dest.with_extension("part").exists());
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
        assert!(!dest.exists());
        assert!(!dest.with_extension("part").exists());
    }

    #[test]
    fn download_verified_wrong_sha1_cleans_part() {
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("file.bin");
        let body = b"abcdefgh";
        let url = serve_body(body);
        let error = download_verified(
            &url,
            &dest,
            body.len() as u64,
            "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        )
        .unwrap_err();
        match error {
            DownloadError::Sha1Mismatch { .. } => {}
            other => panic!("unexpected {other:?}"),
        }
        assert!(error.message().contains("SHA1 incorrecto"));
        assert!(!dest.exists());
        assert!(!dest.with_extension("part").exists());
    }

    #[test]
    fn available_space_on_tempdir() {
        let tmp = TempDir::new().unwrap();
        let free = available_space(tmp.path()).expect("space");
        assert!(free > 0);
        assert_eq!(MIN_FREE_BYTES, 2 * 1024 * 1024 * 1024);
    }
}
