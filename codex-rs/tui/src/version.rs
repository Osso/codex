/// The machine-readable Codex version used for update checks and version gates.
pub const CODEX_CLI_VERSION: &str = "0.120.0";

/// The displayed Codex CLI version for this local build line.
pub const CODEX_CLI_DISPLAY_VERSION: &str = "0.120.0-osso";

#[cfg(test)]
mod tests {
    use super::CODEX_CLI_DISPLAY_VERSION;
    use super::CODEX_CLI_VERSION;

    #[test]
    fn cli_version_is_plain_semver_for_update_checks() {
        let segments = CODEX_CLI_VERSION
            .split('.')
            .map(str::parse::<u64>)
            .collect::<Result<Vec<_>, _>>();

        assert!(
            segments.is_ok(),
            "CODEX_CLI_VERSION must stay plain semver for update checks, got {CODEX_CLI_VERSION}"
        );
    }

    #[test]
    fn display_version_keeps_local_branding() {
        assert_eq!(
            CODEX_CLI_DISPLAY_VERSION,
            format!("{CODEX_CLI_VERSION}-osso")
        );
    }
}
