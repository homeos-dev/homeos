use std::io::Write;
use std::time::Duration;

const FETCH_TIMEOUT_MS: u64 = 1500;
const RELEASES_URL: &str = "https://api.github.com/repos/hainet50b/homeos/releases/latest";
const UPDATE_URL: &str = "https://github.com/hainet50b/homeos";
const SKIP_ENV_VAR: &str = "HOMEOS_SKIP_UPDATE_CHECK";

fn current_tag() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

/// Best-effort, stateless update check. Asks the GitHub releases API for the
/// latest tag on every call and emits one line to `writer` (stderr in
/// production) when that tag is strictly newer than the running binary. No
/// cache is read or written, so the check needs no data directory and behaves
/// the same before and after `homeos init`. Honors `HOMEOS_SKIP_UPDATE_CHECK`
/// (any non-empty value skips the network call entirely). Any failure —
/// timeout, DNS, unparseable tag — is silent; the injectable `writer` lets the
/// `homeos agents-md` caller keep the notice off stdout.
pub(crate) fn check_and_notify_to_writer<W: Write>(
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    if skip_check() {
        return Ok(());
    }
    check_and_notify_to(writer, default_fetch)
}

fn skip_check() -> bool {
    std::env::var_os(SKIP_ENV_VAR).is_some_and(|v| !v.is_empty())
}

fn default_fetch() -> Option<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(FETCH_TIMEOUT_MS)))
        .build()
        .into();
    let response: serde_json::Value = agent
        .get(RELEASES_URL)
        .header("User-Agent", "homeos")
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    response.get("tag_name")?.as_str().map(|s| s.to_string())
}

fn check_and_notify_to<W, F>(writer: &mut W, fetch: F) -> Result<(), Box<dyn std::error::Error>>
where
    W: Write,
    F: FnOnce() -> Option<String>,
{
    let Some(latest_tag) = fetch() else {
        return Ok(());
    };
    if is_update_available(&latest_tag, &current_tag()) {
        writeln!(
            writer,
            "homeos: {latest_tag} available — update at {UPDATE_URL}"
        )?;
    }
    Ok(())
}

/// Parse a `vX.Y.Z` release tag into a numeric `(major, minor, patch)` triple.
/// Returns `None` for anything that does not match the exact form (missing `v`
/// prefix, wrong component count, or non-numeric components).
fn parse_tag(tag: &str) -> Option<(u64, u64, u64)> {
    let rest = tag.strip_prefix('v')?;
    let mut parts = rest.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// True when `latest` is a strictly newer release than `current`, comparing
/// `vX.Y.Z` numerically. Any unparseable tag yields `false` — the check is
/// best-effort and silence beats a false alarm.
fn is_update_available(latest: &str, current: &str) -> bool {
    match (parse_tag(latest), parse_tag(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env_test::EnvVarGuard;

    #[test]
    fn test_skip_env_var_skips_the_network_call() {
        // Arrange
        let guard = EnvVarGuard::capture(SKIP_ENV_VAR);
        guard.set("1");
        let mut writer: Vec<u8> = Vec::new();

        // Act
        check_and_notify_to_writer(&mut writer).unwrap();

        // Assert — nothing fetched, nothing emitted
        assert!(skip_check());
        assert!(writer.is_empty());
    }

    #[test]
    fn test_empty_skip_env_var_does_not_skip_the_check() {
        // Arrange
        let guard = EnvVarGuard::capture(SKIP_ENV_VAR);
        guard.set("");

        // Act
        let skipped = skip_check();

        // Assert
        assert!(!skipped);
    }

    #[test]
    fn test_notice_format_matches_spec() {
        // Arrange
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || Some("v99.0.0".to_string());

        // Act
        check_and_notify_to(&mut writer, fetch).unwrap();

        // Assert — single line `homeos: v<latest> available — update at <url>`.
        let notice = String::from_utf8(writer).unwrap();
        assert_eq!(
            notice,
            format!("homeos: v99.0.0 available — update at {UPDATE_URL}\n")
        );
    }

    #[test]
    fn test_latest_newer_notifies() {
        // Arrange — fetched tag is one patch ahead of the current binary.
        let mut writer: Vec<u8> = Vec::new();
        let current = current_tag();
        let (major, minor, patch) = parse_tag(&current).unwrap();
        let newer = format!("v{major}.{minor}.{}", patch + 1);
        let newer_for_fetch = newer.clone();
        let fetch = move || Some(newer_for_fetch.clone());

        // Act
        check_and_notify_to(&mut writer, fetch).unwrap();

        // Assert
        let notice = String::from_utf8(writer).unwrap();
        assert!(
            notice.contains(&format!("{newer} available")),
            "a strictly newer tag must notify, got: {notice:?}"
        );
    }

    #[test]
    fn test_equal_tag_is_silent() {
        // Arrange — the release API reports exactly the running binary's tag.
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || Some(current_tag());

        // Act
        check_and_notify_to(&mut writer, fetch).unwrap();

        // Assert
        assert!(writer.is_empty(), "equal tag must not notify");
    }

    #[test]
    fn test_latest_older_than_current_is_silent() {
        // Arrange — the reported tag is older than the running binary.
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || Some("v0.0.1".to_string());

        // Act
        check_and_notify_to(&mut writer, fetch).unwrap();

        // Assert
        assert!(writer.is_empty(), "an older tag must not notify");
    }

    #[test]
    fn test_unparseable_fetched_tag_is_silent() {
        // Arrange — fetch returns a tag that is not in vX.Y.Z form.
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || Some("nightly".to_string());

        // Act
        check_and_notify_to(&mut writer, fetch).unwrap();

        // Assert
        assert!(writer.is_empty(), "an unparseable tag must not notify");
    }

    #[test]
    fn test_fetch_failure_is_silent() {
        // Arrange — timeout / DNS / parse failure.
        let mut writer: Vec<u8> = Vec::new();
        let fetch = || None;

        // Act
        check_and_notify_to(&mut writer, fetch).unwrap();

        // Assert
        assert!(writer.is_empty(), "a failed fetch must not notify");
    }

    #[test]
    fn test_parse_tag_accepts_well_formed() {
        // Arrange / Act / Assert
        assert_eq!(parse_tag("v0.3.12"), Some((0, 3, 12)));
        assert_eq!(parse_tag("v10.20.30"), Some((10, 20, 30)));
    }

    #[test]
    fn test_parse_tag_rejects_malformed() {
        // Arrange / Act / Assert
        assert_eq!(parse_tag("0.3.12"), None); // missing v prefix
        assert_eq!(parse_tag("v0.3"), None); // too few components
        assert_eq!(parse_tag("v0.3.12.1"), None); // too many components
        assert_eq!(parse_tag("vx.y.z"), None); // non-numeric
        assert_eq!(parse_tag("nightly"), None); // not a version at all
    }

    #[test]
    fn test_is_update_available_compares_strictly_newer() {
        // Arrange / Act / Assert
        assert!(is_update_available("v0.3.13", "v0.3.12"));
        assert!(is_update_available("v0.4.0", "v0.3.12"));
        assert!(is_update_available("v1.0.0", "v0.3.12"));
        assert!(!is_update_available("v0.3.12", "v0.3.12")); // equal
        assert!(!is_update_available("v0.3.11", "v0.3.12")); // older
        assert!(!is_update_available("garbage", "v0.3.12")); // unparseable latest
        assert!(!is_update_available("v0.3.13", "garbage")); // unparseable current
    }
}
