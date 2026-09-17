//! Índice oficial de builds CEF (`index.json`) y selección de candidato.

use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;

use super::version::CefVersion;

const FETCH_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Deserialize, Clone, Debug)]
pub struct IndexFile {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct IndexVersion {
    pub cef_version: String,
    pub chromium_version: String,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub files: Vec<IndexFile>,
}

/// Índice por clave de plataforma (`linux64`, `windows64`, …).
#[derive(Clone, Debug, Default)]
pub struct CefIndex {
    platforms: HashMap<String, Vec<IndexVersion>>,
}

impl CefIndex {
    pub fn versions_for(&self, platform: &str) -> &[IndexVersion] {
        self.platforms
            .get(platform)
            .map(|versions| versions.as_slice())
            .unwrap_or(&[])
    }
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub version: IndexVersion,
    pub file: IndexFile,
}

#[derive(Debug)]
pub enum IndexFetch {
    NotModified,
    Fetched { body: String, etag: Option<String> },
}

/// Parsea el `index.json` de Spotify CDN. Tolera campos desconocidos.
pub fn parse_index(json: &str) -> Result<CefIndex, String> {
    let root: HashMap<String, serde_json::Value> = serde_json::from_str(json)
        .map_err(|error| format!("No se pudo parsear el índice CEF: {error}"))?;

    let mut platforms = HashMap::new();
    for (platform, value) in root {
        let Some(versions_value) = value.get("versions") else {
            continue;
        };
        let Some(raw_versions) = versions_value.as_array() else {
            continue;
        };
        let mut versions = Vec::new();
        for entry in raw_versions {
            if let Ok(version) = serde_json::from_value::<IndexVersion>(entry.clone()) {
                versions.push(version);
            }
        }
        platforms.insert(platform, versions);
    }

    Ok(CefIndex { platforms })
}

/// Elige el mayor `stable` con archivo `minimal`, estrictamente más nuevo que
/// `current` y que no esté en la denylist. Omite versiones que no parsean.
pub fn select_candidate(
    index: &CefIndex,
    platform: &str,
    current: &CefVersion,
    is_denylisted: &dyn Fn(&str) -> bool,
) -> Option<Candidate> {
    let mut best: Option<(CefVersion, Candidate)> = None;

    for version in index.versions_for(platform) {
        if version.channel != "stable" {
            continue;
        }
        if is_denylisted(&version.cef_version) {
            continue;
        }
        let Ok(parsed) = CefVersion::parse(&version.cef_version) else {
            continue;
        };
        if parsed <= *current {
            continue;
        }
        let Some(file) = version.files.iter().find(|file| file.kind == "minimal") else {
            continue;
        };

        let candidate = Candidate {
            version: version.clone(),
            file: file.clone(),
        };
        match &best {
            Some((best_ver, _)) if parsed <= *best_ver => {}
            _ => best = Some((parsed, candidate)),
        }
    }

    best.map(|(_, candidate)| candidate)
}

/// URL de descarga: `base` + nombre con `+` y el resto no-unreserved percent-encoded.
pub fn download_url(base_url: &str, file_name: &str) -> String {
    let encoded = percent_encode(file_name);
    if base_url.ends_with('/') {
        format!("{base_url}{encoded}")
    } else {
        format!("{base_url}/{encoded}")
    }
}

/// GET bloqueante con gzip y `If-None-Match`. 304 → `NotModified`.
pub fn fetch_index(url: &str, etag: Option<&str>) -> Result<IndexFetch, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .gzip(true)
        .build()
        .map_err(|error| format!("No se pudo crear el cliente HTTP: {error}"))?;

    let mut request = client.get(url).header("Accept-Encoding", "gzip");
    if let Some(etag) = etag.filter(|value| !value.is_empty()) {
        request = request.header("If-None-Match", etag);
    }

    let response = request
        .send()
        .map_err(|error| format!("No se pudo descargar el índice CEF: {error}"))?;

    let status = response.status();
    if status == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(IndexFetch::NotModified);
    }
    if !status.is_success() {
        return Err(format!(
            "El índice CEF respondió {}: {url}",
            status.as_u16()
        ));
    }

    let etag = response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = response
        .text()
        .map_err(|error| format!("No se pudo leer el índice CEF: {error}"))?;
    Ok(IndexFetch::Fetched { body, etag })
}

fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        if is_unreserved(byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn is_unreserved(byte: u8) -> bool {
    matches!(
        byte,
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    const SAMPLE: &str = r#"{
      "linux64": {
        "versions": [
          {
            "cef_version": "152.0.6+g708dc14+chromium-152.0.7977.83",
            "chromium_version": "152.0.7977.83",
            "channel": "stable",
            "files": [
              {
                "type": "minimal",
                "name": "cef_binary_152.0.6+g708dc14+chromium-152.0.7977.83_linux64_minimal.tar.bz2",
                "sha1": "9711b86c105fb590da576fe5a829802f1a79d520",
                "size": 321503907,
                "last_modified": "2026-09-07T00:00:00Z"
              },
              {
                "type": "standard",
                "name": "cef_binary_standard.tar.bz2",
                "sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "size": 1
              }
            ]
          },
          {
            "cef_version": "153.0.1+gabc+chromium-153.0.8000.10",
            "chromium_version": "153.0.8000.10",
            "channel": "stable",
            "files": [
              {
                "type": "minimal",
                "name": "cef_binary_153.0.1+gabc+chromium-153.0.8000.10_linux64_minimal.tar.bz2",
                "sha1": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "size": 2
              }
            ]
          },
          {
            "cef_version": "154.0.0+gfff+chromium-154.0.1.1",
            "chromium_version": "154.0.1.1",
            "channel": "beta",
            "files": [
              {
                "type": "minimal",
                "name": "cef_binary_154_minimal.tar.bz2",
                "sha1": "cccccccccccccccccccccccccccccccccccccccc",
                "size": 3
              }
            ]
          },
          {
            "cef_version": "not-a-version",
            "chromium_version": "x",
            "channel": "stable",
            "files": [
              {
                "type": "minimal",
                "name": "bad.tar.bz2",
                "sha1": "dddddddddddddddddddddddddddddddddddddddd",
                "size": 4
              }
            ]
          },
          {
            "cef_version": "155.0.0+ggg+chromium-155.0.1.1",
            "chromium_version": "155.0.1.1",
            "channel": "stable",
            "files": [
              {
                "type": "minimal",
                "name": "denied.tar.bz2",
                "sha1": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                "size": 5
              }
            ]
          },
          {
            "cef_version": "156.0.0+ghh+chromium-156.0.1.1",
            "chromium_version": "156.0.1.1",
            "channel": "stable",
            "files": [
              {
                "type": "standard",
                "name": "no-minimal.tar.bz2",
                "sha1": "ffffffffffffffffffffffffffffffffffffffff",
                "size": 6
              }
            ]
          },
          {
            "cef_version": "151.0.1+gold+chromium-151.0.1.1",
            "chromium_version": "151.0.1.1",
            "channel": "stable",
            "files": [
              {
                "type": "minimal",
                "name": "older.tar.bz2",
                "sha1": "1111111111111111111111111111111111111111",
                "size": 7
              }
            ]
          }
        ]
      },
      "windows64": { "versions": [] },
      "comment": "not a platform object"
    }"#;

    fn current() -> CefVersion {
        CefVersion::parse("152.0.6+g708dc14+chromium-152.0.7977.83").unwrap()
    }

    #[test]
    fn parse_index_tolerates_unknown_fields() {
        let index = parse_index(SAMPLE).expect("parse");
        assert_eq!(index.versions_for("linux64").len(), 7);
        assert!(index.versions_for("windows64").is_empty());
        assert!(index.versions_for("macosx64").is_empty());
        let first = &index.versions_for("linux64")[0];
        assert_eq!(first.channel, "stable");
        assert_eq!(first.files[0].kind, "minimal");
        assert_eq!(first.files[0].size, 321503907);
    }

    #[test]
    fn parse_index_rejects_garbage() {
        let error = parse_index("not-json").unwrap_err();
        assert!(error.contains("No se pudo parsear el índice CEF"));
    }

    #[test]
    fn select_candidate_picks_greatest_stable_minimal() {
        let index = parse_index(SAMPLE).unwrap();
        let chosen =
            select_candidate(&index, "linux64", &current(), &|_| false).expect("candidate");
        assert_eq!(chosen.version.cef_version, "155.0.0+ggg+chromium-155.0.1.1");
        assert_eq!(chosen.file.name, "denied.tar.bz2");
    }

    #[test]
    fn select_candidate_skips_denylist_beta_unparsable_and_older() {
        let index = parse_index(SAMPLE).unwrap();
        let chosen = select_candidate(&index, "linux64", &current(), &|cef| {
            cef.starts_with("155.")
        })
        .expect("candidate");
        assert_eq!(
            chosen.version.cef_version,
            "153.0.1+gabc+chromium-153.0.8000.10"
        );
        assert_eq!(chosen.file.kind, "minimal");
    }

    #[test]
    fn select_candidate_none_when_equal_to_newest_stable() {
        let index = parse_index(SAMPLE).unwrap();
        let current = CefVersion::parse("155.0.0+ggg+chromium-155.0.1.1").unwrap();
        assert!(select_candidate(&index, "linux64", &current, &|_| false).is_none());
    }

    #[test]
    fn select_candidate_none_when_no_newer() {
        let index = parse_index(SAMPLE).unwrap();
        let newest = CefVersion::parse("200.0.0+g+chromium-200.0.0.0").unwrap();
        assert!(select_candidate(&index, "linux64", &newest, &|_| false).is_none());
        assert!(select_candidate(&index, "macosarm64", &current(), &|_| false).is_none());
    }

    #[test]
    fn download_url_percent_encodes_plus_and_other_reserved() {
        let url = download_url(
            "https://cef-builds.spotifycdn.com/",
            "cef_binary_152.0.6+g708dc14+chromium-152.0.7977.83_linux64_minimal.tar.bz2",
        );
        assert_eq!(
            url,
            "https://cef-builds.spotifycdn.com/cef_binary_152.0.6%2Bg708dc14%2Bchromium-152.0.7977.83_linux64_minimal.tar.bz2"
        );
        assert!(!url.contains('+'));
        let joined = download_url("https://example.com/cef", "a b");
        assert_eq!(joined, "https://example.com/cef/a%20b");
    }

    fn serve_once(status_line: &str, headers: &str, body: &[u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let status_line = status_line.to_string();
        let headers = headers.to_string();
        let body = body.to_vec();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let response = format!(
                "{status_line}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(&body);
        });
        format!("http://{addr}/index.json")
    }

    #[test]
    fn fetch_index_fetched_and_not_modified() {
        let body = br#"{"linux64":{"versions":[]}}"#;
        let url = serve_once(
            "HTTP/1.1 200 OK",
            "ETag: \"abc123\"\r\nContent-Type: application/json\r\n",
            body,
        );
        match fetch_index(&url, None).expect("fetch") {
            IndexFetch::Fetched { body: text, etag } => {
                assert!(text.contains("linux64"));
                assert_eq!(etag.as_deref(), Some("\"abc123\""));
            }
            IndexFetch::NotModified => panic!("expected body"),
        }

        let url = serve_once("HTTP/1.1 304 Not Modified", "ETag: \"abc123\"\r\n", b"");
        match fetch_index(&url, Some("\"abc123\"")).expect("304") {
            IndexFetch::NotModified => {}
            IndexFetch::Fetched { .. } => panic!("expected not modified"),
        }
    }

    #[test]
    fn fetch_index_http_error() {
        let url = serve_once("HTTP/1.1 500 Internal Server Error", "", b"nope");
        let error = fetch_index(&url, None).unwrap_err();
        assert!(error.contains("500"));
    }
}
