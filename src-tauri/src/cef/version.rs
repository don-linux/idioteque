//! Versión de un runtime CEF (`MAJOR.MINOR.PATCH+commit+chromium-a.b.c.d`).

use std::fmt;
use std::str::FromStr;

/// Versión de CEF parseada según el contrato 3.5.
#[derive(Debug, Clone)]
pub struct CefVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub commit: String,
    pub chromium: [u32; 4],
}

impl CefVersion {
    /// Parsea cadenas como `152.0.6+g708dc14+chromium-152.0.7977.83`.
    pub fn parse(input: &str) -> Result<CefVersion, String> {
        parse_version(input)
    }
}

impl FromStr for CefVersion {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_version(s)
    }
}

impl fmt::Display for CefVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}+{}+chromium-{}.{}.{}.{}",
            self.major,
            self.minor,
            self.patch,
            self.commit,
            self.chromium[0],
            self.chromium[1],
            self.chromium[2],
            self.chromium[3]
        )
    }
}

impl PartialEq for CefVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for CefVersion {}

impl PartialOrd for CefVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CefVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.major, self.minor, self.patch, self.chromium).cmp(&(
            other.major,
            other.minor,
            other.patch,
            other.chromium,
        ))
    }
}

/// Extrae `152.0.7977.83` de una cadena de versión CEF.
pub fn chromium_from(input: &str) -> Option<String> {
    let rest = input.split("chromium-").nth(1)?;
    let token = rest
        .split(|c: char| c == '+' || c.is_whitespace())
        .next()?
        .trim();
    parse_chromium(token).ok()?;
    Some(token.to_string())
}

fn parse_version(input: &str) -> Result<CefVersion, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("versión CEF vacía".to_string());
    }

    let mut parts = input.split('+');
    let triple = parts
        .next()
        .ok_or_else(|| format!("versión CEF inválida: `{input}`"))?;
    let numbers = parse_triple(triple)
        .map_err(|_| format!("versión CEF inválida: `{input}` (se esperaba MAJOR.MINOR.PATCH)"))?;

    let rest: Vec<&str> = parts.collect();
    if rest.is_empty() {
        return Err(format!(
            "versión CEF inválida: `{input}` (falta +commit+chromium-…)"
        ));
    }

    let mut commit = String::new();
    let mut chromium = None;
    for part in rest {
        if let Some(token) = part.strip_prefix("chromium-") {
            chromium = Some(
                parse_chromium(token)
                    .map_err(|_| format!("versión Chromium inválida en `{input}`"))?,
            );
        } else if commit.is_empty() {
            commit = part.to_string();
        }
    }

    let chromium = chromium
        .ok_or_else(|| format!("versión CEF inválida: `{input}` (falta chromium-a.b.c.d)"))?;

    Ok(CefVersion {
        major: numbers[0],
        minor: numbers[1],
        patch: numbers[2],
        commit,
        chromium,
    })
}

fn parse_triple(input: &str) -> Result<[u32; 3], ()> {
    let mut nums = [0u32; 3];
    let mut parts = input.split('.');
    for slot in &mut nums {
        let part = parts.next().ok_or(())?;
        *slot = part.parse().map_err(|_| ())?;
    }
    if parts.next().is_some() {
        return Err(());
    }
    Ok(nums)
}

fn parse_chromium(input: &str) -> Result<[u32; 4], ()> {
    let mut nums = [0u32; 4];
    let mut parts = input.split('.');
    for slot in &mut nums {
        let part = parts.next().ok_or(())?;
        if part.is_empty() {
            return Err(());
        }
        *slot = part.parse().map_err(|_| ())?;
    }
    if parts.next().is_some() {
        return Err(());
    }
    Ok(nums)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "152.0.6+g708dc14+chromium-152.0.7977.83";

    #[test]
    fn parse_round_trips_sample() {
        let parsed = CefVersion::parse(SAMPLE).expect("parse");
        assert_eq!(parsed.major, 152);
        assert_eq!(parsed.minor, 0);
        assert_eq!(parsed.patch, 6);
        assert_eq!(parsed.commit, "g708dc14");
        assert_eq!(parsed.chromium, [152, 0, 7977, 83]);
        assert_eq!(parsed.to_string(), SAMPLE);
    }

    #[test]
    fn chromium_from_extracts_four_tuple() {
        assert_eq!(chromium_from(SAMPLE).as_deref(), Some("152.0.7977.83"));
        assert_eq!(chromium_from("nope"), None);
        assert_eq!(chromium_from("152.0.6+g+chromium-1.2.3"), None);
    }

    #[test]
    fn ordering_uses_triple_then_chromium() {
        let a = CefVersion::parse("152.0.6+ga+chromium-152.0.7977.83").unwrap();
        let b = CefVersion::parse("152.0.7+gb+chromium-152.0.7977.1").unwrap();
        let c = CefVersion::parse("152.0.6+gz+chromium-152.0.7977.84").unwrap();
        let d = CefVersion::parse("153.0.1+gabc+chromium-153.0.8000.10").unwrap();
        let same_numbers = CefVersion::parse("152.0.6+other+chromium-152.0.7977.83").unwrap();

        assert!(a < b);
        assert!(a < c);
        assert!(c < b);
        assert!(a < d);
        assert_eq!(a, same_numbers);
        assert!(a.cmp(&same_numbers) == std::cmp::Ordering::Equal);
    }

    #[test]
    fn malformed_input_is_err() {
        assert!(CefVersion::parse("").is_err());
        assert!(CefVersion::parse("foo").is_err());
        assert!(CefVersion::parse("152.0").is_err());
        assert!(CefVersion::parse("152.0.6").is_err());
        assert!(CefVersion::parse("152.0.6+g708dc14").is_err());
        assert!(CefVersion::parse("152.0.6+g+chromium-1.2.3").is_err());
        assert!(CefVersion::parse("152.0.6+g+chromium-152.0.7977.83.1").is_err());
    }
}
