//! Load user rule files from a `rules/` directory.
//!
//! Rule files are `*.md` files read in sorted filename order for deterministic
//! concatenation, mirroring the behaviour of `~/.claude/rules/` in Claude Code.

use std::path::Path;

/// Reads all `*.md` files in `rules_dir`, sorted by filename, and returns
/// their trimmed contents joined by double newlines. Returns `None` when
/// no non-empty rule files are found.
pub(crate) fn load_rules_from_dir(rules_dir: &Path) -> Option<String> {
    if !rules_dir.is_dir() {
        return None;
    }

    let entries = std::fs::read_dir(rules_dir).ok()?;
    let mut paths: Vec<std::path::PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "md"))
        .collect();
    paths.sort();

    let parts: Vec<String> = paths
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .map(|c| c.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn no_dir_returns_none() {
        let tmp = tempdir().unwrap();
        let missing = tmp.path().join("nonexistent");
        assert!(load_rules_from_dir(&missing).is_none());
    }

    #[test]
    fn empty_dir_returns_none() {
        let tmp = tempdir().unwrap();
        let rules = tmp.path().join("rules");
        std::fs::create_dir(&rules).unwrap();
        assert!(load_rules_from_dir(&rules).is_none());
    }

    #[test]
    fn loads_md_files_sorted_by_name() {
        let tmp = tempdir().unwrap();
        let rules = tmp.path().join("rules");
        std::fs::create_dir(&rules).unwrap();
        std::fs::write(rules.join("02-second.md"), "second rule").unwrap();
        std::fs::write(rules.join("01-first.md"), "first rule").unwrap();
        std::fs::write(rules.join("skip.txt"), "not markdown").unwrap();

        let result = load_rules_from_dir(&rules).unwrap();
        assert_eq!(result, "first rule\n\nsecond rule");
    }

    #[test]
    fn skips_empty_files() {
        let tmp = tempdir().unwrap();
        let rules = tmp.path().join("rules");
        std::fs::create_dir(&rules).unwrap();
        std::fs::write(rules.join("01-empty.md"), "  \n  ").unwrap();
        std::fs::write(rules.join("02-real.md"), "actual content").unwrap();

        let result = load_rules_from_dir(&rules).unwrap();
        assert_eq!(result, "actual content");
    }
}
