//! Parser for `git status --porcelain=v2 --branch -z`.
//!
//! VS Code still reads porcelain v1 (`status -z`). v2 is the same idea —
//! machine records, never the human `git status` — with branch headers
//! in the same stream. Records are NUL-terminated; rename paths come as
//! the next record.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitFileKind {
    Ordinary,
    Renamed,
    Unmerged,
    Untracked,
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFile {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
    /// Index column of `XY`. `"."` means unmodified in the index.
    pub staged: String,
    /// Worktree column of `XY`. `"."` means unmodified on disk.
    pub unstaged: String,
    pub kind: GitFileKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedStatus {
    pub oid: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
    pub initial: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub files: Vec<GitFile>,
}

pub fn parse_status_v2(stdout: &[u8]) -> Result<ParsedStatus, String> {
    let mut parsed = ParsedStatus::default();
    let mut records = stdout.split(|byte| *byte == 0);
    let mut expect_rename_source = false;

    while let Some(raw) = records.next() {
        if raw.is_empty() {
            continue;
        }

        let record = String::from_utf8_lossy(raw);

        if expect_rename_source {
            if let Some(file) = parsed.files.last_mut() {
                file.original_path = Some(record.into_owned());
            }
            expect_rename_source = false;
            continue;
        }

        if let Some(header) = record.strip_prefix("# ") {
            parse_header(&mut parsed, header);
            continue;
        }

        if let Some(path) = record.strip_prefix("? ") {
            parsed.files.push(GitFile {
                path: path.to_string(),
                original_path: None,
                staged: ".".to_string(),
                unstaged: "?".to_string(),
                kind: GitFileKind::Untracked,
            });
            continue;
        }

        if let Some(path) = record.strip_prefix("! ") {
            parsed.files.push(GitFile {
                path: path.to_string(),
                original_path: None,
                staged: ".".to_string(),
                unstaged: "!".to_string(),
                kind: GitFileKind::Ignored,
            });
            continue;
        }

        if let Some(rest) = record.strip_prefix("1 ") {
            parsed.files.push(parse_changed(rest, GitFileKind::Ordinary)?);
            continue;
        }

        if let Some(rest) = record.strip_prefix("2 ") {
            parsed
                .files
                .push(parse_changed(rest, GitFileKind::Renamed)?);
            expect_rename_source = true;
            continue;
        }

        if let Some(rest) = record.strip_prefix("u ") {
            parsed
                .files
                .push(parse_unmerged(rest)?);
            continue;
        }
    }

    Ok(parsed)
}

fn parse_header(parsed: &mut ParsedStatus, header: &str) {
    if let Some(value) = header.strip_prefix("branch.oid ") {
        if value == "(initial)" {
            parsed.initial = true;
            parsed.oid = None;
        } else {
            parsed.oid = Some(value.to_string());
        }
        return;
    }

    if let Some(value) = header.strip_prefix("branch.head ") {
        if value == "(detached)" {
            parsed.detached = true;
            parsed.branch = None;
        } else {
            parsed.branch = Some(value.to_string());
        }
        return;
    }

    if let Some(value) = header.strip_prefix("branch.upstream ") {
        parsed.upstream = Some(value.to_string());
        return;
    }

    if let Some(value) = header.strip_prefix("branch.ab ") {
        if let Some((ahead, behind)) = parse_ahead_behind(value) {
            parsed.ahead = ahead;
            parsed.behind = behind;
        }
    }
}

fn parse_ahead_behind(value: &str) -> Option<(u32, u32)> {
    let (ahead, behind) = value.split_once(' ')?;
    let ahead = ahead.strip_prefix('+')?.parse().ok()?;
    let behind = behind.strip_prefix('-')?.parse().ok()?;
    Some((ahead, behind))
}

fn parse_changed(rest: &str, kind: GitFileKind) -> Result<GitFile, String> {
    // ordinary: XY sub mH mI mW hH hI path  → skip 7, rest is path
    // renamed:  XY sub mH mI mW hH hI score path
    let field_count = match kind {
        GitFileKind::Renamed => 8,
        _ => 7,
    };

    let Some((xy, path)) = split_after(rest, field_count) else {
        return Err(format!("Registro porcelain v2 incompleto: {rest}"));
    };

    let (staged, unstaged) = xy_columns(xy)?;

    Ok(GitFile {
        path: path.to_string(),
        original_path: None,
        staged,
        unstaged,
        kind,
    })
}

fn parse_unmerged(rest: &str) -> Result<GitFile, String> {
    // XY sub m1 m2 m3 mW h1 h2 h3 path
    let Some((xy, path)) = split_after(rest, 9) else {
        return Err(format!("Registro de conflicto incompleto: {rest}"));
    };

    let (staged, unstaged) = xy_columns(xy)?;

    Ok(GitFile {
        path: path.to_string(),
        original_path: None,
        staged,
        unstaged,
        kind: GitFileKind::Unmerged,
    })
}

fn xy_columns(xy: &str) -> Result<(String, String), String> {
    let mut chars = xy.chars();
    let staged = chars.next();
    let unstaged = chars.next();

    match (staged, unstaged, chars.next()) {
        (Some(staged), Some(unstaged), None) => Ok((staged.to_string(), unstaged.to_string())),
        _ => Err(format!("XY porcelain inválido: {xy}")),
    }
}

/// Skip `n` space-separated fields; the remainder is the path (may contain spaces).
fn split_after(record: &str, fields: usize) -> Option<(&str, &str)> {
    let mut rest = record;

    for _ in 0..fields {
        let (field, tail) = rest.split_once(' ')?;
        if field.is_empty() {
            return None;
        }
        rest = tail;
    }

    if rest.is_empty() {
        return None;
    }

    let first = record.split_once(' ')?.0;
    Some((first, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> ParsedStatus {
        parse_status_v2(text.as_bytes()).expect("parse")
    }

    fn nul_join(records: &[&str]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for record in records {
            bytes.extend_from_slice(record.as_bytes());
            bytes.push(0);
        }
        bytes
    }

    #[test]
    fn parse_dirty_and_untracked_sample() {
        let stdout = nul_join(&[
            "# branch.oid 3e50f2b32f1652c49a0ae75ecac9af0cfc04d8e0",
            "# branch.head main",
            "1 .M N... 100644 100644 100644 ce013625030ba8dba906f756967f9e9ca394464a ce013625030ba8dba906f756967f9e9ca394464a a.md",
            "? b.md",
        ]);

        let parsed = parse_status_v2(&stdout).expect("parse");
        assert_eq!(parsed.branch.as_deref(), Some("main"));
        assert_eq!(
            parsed.oid.as_deref(),
            Some("3e50f2b32f1652c49a0ae75ecac9af0cfc04d8e0")
        );
        assert!(!parsed.detached);
        assert!(!parsed.initial);
        assert_eq!(parsed.files.len(), 2);
        assert_eq!(parsed.files[0].path, "a.md");
        assert_eq!(parsed.files[0].staged, ".");
        assert_eq!(parsed.files[0].unstaged, "M");
        assert_eq!(parsed.files[0].kind, GitFileKind::Ordinary);
        assert_eq!(parsed.files[1].path, "b.md");
        assert_eq!(parsed.files[1].kind, GitFileKind::Untracked);
        assert_eq!(parsed.files[1].staged, ".");
        assert_eq!(parsed.files[1].unstaged, "?");
    }

    #[test]
    fn parse_rename_uses_the_next_nul_record() {
        let stdout = nul_join(&[
            "# branch.head main",
            "2 RM N... 100644 100644 100644 ce013625030ba8dba906f756967f9e9ca394464a ce013625030ba8dba906f756967f9e9ca394464a R100 renamed.md",
            "a.md",
            "? b.md",
        ]);

        let parsed = parse_status_v2(&stdout).expect("parse");
        assert_eq!(parsed.files[0].kind, GitFileKind::Renamed);
        assert_eq!(parsed.files[0].path, "renamed.md");
        assert_eq!(parsed.files[0].original_path.as_deref(), Some("a.md"));
        assert_eq!(parsed.files[0].staged, "R");
        assert_eq!(parsed.files[0].unstaged, "M");
        assert_eq!(parsed.files[1].path, "b.md");
    }

    #[test]
    fn parse_initial_detached_and_ahead_behind() {
        let parsed = parse(
            "# branch.oid (initial)\0# branch.head (detached)\0# branch.upstream origin/main\0# branch.ab +3 -1\0",
        );

        assert!(parsed.initial);
        assert!(parsed.detached);
        assert_eq!(parsed.oid, None);
        assert_eq!(parsed.branch, None);
        assert_eq!(parsed.upstream.as_deref(), Some("origin/main"));
        assert_eq!(parsed.ahead, 3);
        assert_eq!(parsed.behind, 1);
    }

    #[test]
    fn parse_path_with_spaces() {
        let stdout = nul_join(&["1 .M N... 100644 100644 100644 abc def notes/mi nota.md"]);
        let parsed = parse_status_v2(&stdout).expect("parse");
        assert_eq!(parsed.files[0].path, "notes/mi nota.md");
    }

    #[test]
    fn parse_unmerged_conflict() {
        let stdout =
            nul_join(&["u UU N... 100644 100644 100644 100644 aaa bbb ccc docs/conflict.md"]);
        let parsed = parse_status_v2(&stdout).expect("parse");
        assert_eq!(parsed.files[0].kind, GitFileKind::Unmerged);
        assert_eq!(parsed.files[0].path, "docs/conflict.md");
        assert_eq!(parsed.files[0].staged, "U");
        assert_eq!(parsed.files[0].unstaged, "U");
    }

    #[test]
    fn parse_empty_stdout_is_a_clean_status() {
        let parsed = parse_status_v2(b"").expect("empty");
        assert_eq!(parsed, ParsedStatus::default());
    }

    #[test]
    fn parse_headers_only_has_no_files() {
        let parsed = parse("# branch.oid abc\0# branch.head main\0");
        assert_eq!(parsed.branch.as_deref(), Some("main"));
        assert_eq!(parsed.oid.as_deref(), Some("abc"));
        assert!(parsed.files.is_empty());
    }

    #[test]
    fn parse_incomplete_ordinary_record_is_an_error() {
        let error = parse_status_v2(b"1 .M N... 100644\0").expect_err("incomplete");
        assert!(error.contains("incompleto"));
    }

    #[test]
    fn parse_incomplete_unmerged_record_is_an_error() {
        let error = parse_status_v2(b"u UU N... 100644 100644\0").expect_err("incomplete");
        assert!(error.contains("conflicto"));
    }

    #[test]
    fn parse_rejects_xy_that_is_not_two_columns() {
        let short = parse_status_v2(b"1 M N... 100644 100644 100644 abc def a.md\0")
            .expect_err("one-char XY");
        assert!(short.contains("XY porcelain inválido"));

        let long = parse_status_v2(b"1 MMM N... 100644 100644 100644 abc def a.md\0")
            .expect_err("three-char XY");
        assert!(long.contains("XY porcelain inválido"));
    }

    #[test]
    fn parse_rename_without_source_record_does_not_panic() {
        let stdout = nul_join(&["2 R. N... 100644 100644 100644 abc def R100 renamed.md"]);
        let parsed = parse_status_v2(&stdout).expect("rename without source");
        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].kind, GitFileKind::Renamed);
        assert_eq!(parsed.files[0].path, "renamed.md");
        assert_eq!(parsed.files[0].original_path, None);
    }

    #[test]
    fn parse_ignored_and_unknown_records() {
        let stdout = nul_join(&["! build/out", "x mystery", "? keep.md"]);
        let parsed = parse_status_v2(&stdout).expect("parse");
        assert_eq!(parsed.files.len(), 2);
        assert_eq!(parsed.files[0].kind, GitFileKind::Ignored);
        assert_eq!(parsed.files[0].path, "build/out");
        assert_eq!(parsed.files[0].staged, ".");
        assert_eq!(parsed.files[0].unstaged, "!");
        assert_eq!(parsed.files[1].kind, GitFileKind::Untracked);
        assert_eq!(parsed.files[1].path, "keep.md");
        assert_eq!(parsed.files[1].staged, ".");
        assert_eq!(parsed.files[1].unstaged, "?");
    }

    #[test]
    fn malformed_ahead_behind_does_not_clobber_a_valid_one() {
        let parsed = parse("# branch.ab +3 -1\0# branch.ab garbage\0# branch.ab no-plus 1\0");
        assert_eq!(parsed.ahead, 3);
        assert_eq!(parsed.behind, 1);
    }

    #[test]
    fn parse_rename_path_with_spaces() {
        let stdout = nul_join(&[
            "2 R. N... 100644 100644 100644 abc def R100 new file.md",
            "old file.md",
        ]);
        let parsed = parse_status_v2(&stdout).expect("parse");
        assert_eq!(parsed.files[0].path, "new file.md");
        assert_eq!(
            parsed.files[0].original_path.as_deref(),
            Some("old file.md")
        );
    }

    #[test]
    fn parse_incomplete_rename_record_is_an_error() {
        let error = parse_status_v2(b"2 R. N... 100644\0").expect_err("incomplete rename");
        assert!(error.contains("incompleto"));
    }

    #[test]
    fn parse_rejects_xy_on_unmerged_and_rename() {
        let unmerged =
            parse_status_v2(b"u U N... 100644 100644 100644 100644 aaa bbb ccc docs/conflict.md\0")
                .expect_err("one-char XY unmerged");
        assert!(unmerged.contains("XY porcelain inválido"));

        let renamed = parse_status_v2(b"2 RRR N... 100644 100644 100644 abc def R100 renamed.md\0")
            .expect_err("three-char XY rename");
        assert!(renamed.contains("XY porcelain inválido"));
    }

    #[test]
    fn later_detached_header_clears_a_previous_branch() {
        let parsed = parse("# branch.head main\0# branch.head (detached)\0");
        assert!(parsed.detached);
        assert_eq!(parsed.branch, None);
    }

    #[test]
    fn later_initial_header_clears_a_previous_oid() {
        let parsed = parse("# branch.oid abcdef\0# branch.oid (initial)\0");
        assert!(parsed.initial);
        assert_eq!(parsed.oid, None);
    }
}
