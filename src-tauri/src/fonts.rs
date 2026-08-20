use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemFont {
    pub family: String,
    pub monospace: bool,
}

pub fn collect_families(faces: impl IntoIterator<Item = (String, bool)>) -> Vec<SystemFont> {
    let mut by_family: BTreeMap<String, bool> = BTreeMap::new();

    for (family, monospace) in faces {
        let name = family.trim();
        if name.is_empty() {
            continue;
        }

        let entry = by_family.entry(name.to_string()).or_insert(false);
        *entry |= monospace;
    }

    let mut fonts: Vec<SystemFont> = by_family
        .into_iter()
        .map(|(family, monospace)| SystemFont { family, monospace })
        .collect();

    fonts.sort_by(|left, right| {
        right
            .monospace
            .cmp(&left.monospace)
            .then_with(|| {
                left.family
                    .to_ascii_lowercase()
                    .cmp(&right.family.to_ascii_lowercase())
            })
            .then_with(|| left.family.cmp(&right.family))
    });

    fonts
}

fn list_from_database(db: &fontdb::Database) -> Vec<SystemFont> {
    collect_families(db.faces().filter_map(|face| {
        let (family, _) = face.families.first()?;
        Some((family.clone(), face.monospaced))
    }))
}

#[tauri::command]
pub fn list_system_fonts() -> Result<Vec<SystemFont>, String> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    Ok(list_from_database(&db))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_families_dedupes_and_marks_mono() {
        let fonts = collect_families([
            ("JetBrainsMono Nerd Font".into(), true),
            ("JetBrainsMono Nerd Font".into(), false),
            ("Inter".into(), false),
            ("  ".into(), false),
            ("Hack".into(), true),
        ]);

        assert_eq!(
            fonts,
            vec![
                SystemFont {
                    family: "Hack".into(),
                    monospace: true,
                },
                SystemFont {
                    family: "JetBrainsMono Nerd Font".into(),
                    monospace: true,
                },
                SystemFont {
                    family: "Inter".into(),
                    monospace: false,
                },
            ]
        );
    }

    #[test]
    fn collect_families_sorts_mono_then_name() {
        let fonts = collect_families([
            ("Zebra".into(), false),
            ("alpha".into(), false),
            ("Beta Mono".into(), true),
            ("aaa mono".into(), true),
        ]);

        let names: Vec<_> = fonts.iter().map(|font| font.family.as_str()).collect();
        assert_eq!(names, ["aaa mono", "Beta Mono", "alpha", "Zebra"]);
    }
}
