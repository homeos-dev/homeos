use crate::error::{HomeosError, reasons};

/// Validate a package or plugin name against the strict whitelist:
/// `^[a-z0-9][a-z0-9._-]*$`. Rejects empty strings, names starting with
/// a character outside `[a-z0-9]`, names containing characters outside
/// `[a-z0-9._-]`, and names containing the `..` substring.
///
/// This guards against shell injection, path traversal, leading-dash
/// flag confusion, and any other character class that could surprise
/// downstream filesystem or process operations.
pub fn validate_name(name: &str) -> Result<(), HomeosError> {
    if name.is_empty() {
        return Err(HomeosError::new(
            reasons::VALIDATION_ERROR,
            "Name must not be empty",
        ));
    }

    let first = name.chars().next().unwrap();
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return Err(HomeosError::new(
            reasons::VALIDATION_ERROR,
            format!("Name '{name}' must start with a lowercase letter or digit"),
        ));
    }

    for c in name.chars() {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_' || c == '-') {
            return Err(HomeosError::new(
                reasons::VALIDATION_ERROR,
                format!("Name '{name}' contains invalid character. Allowed: [a-z0-9._-]"),
            ));
        }
    }

    if name.contains("..") {
        return Err(HomeosError::new(
            reasons::VALIDATION_ERROR,
            format!("Name '{name}' must not contain '..'"),
        ));
    }

    Ok(())
}

/// Allowed URL schemes for `homeos init <url>` and `homeos plugin add <url>`.
const ALLOWED_URL_SCHEMES: &[&str] = &["http", "https", "git", "ssh", "git+ssh"];

/// Validate a URL passed to `homeos init <url>` or `homeos plugin add <url>`.
///
/// Accepts two forms: scheme-full `scheme://…` (scheme one of `http`,
/// `https`, `git`, `ssh`, `git+ssh`) and SCP-like `user@host:path` (the form
/// GitHub's SSH clone button produces), validated per component by
/// `validate_scp_like`.
///
/// Rejects in both forms:
/// - empty input
/// - ASCII control characters (including NUL, CR, LF, tab)
/// - percent-encoded NUL bytes (`%00`, case-insensitive)
/// - percent-encoded `..` (`%2e%2e`, case-insensitive)
/// - any `?` (query string) — git clone URLs have no legitimate query string,
///   so the presence of one indicates injection or misuse
/// - unknown schemes and bare filesystem paths (no scheme, no `@`)
pub fn validate_url(url: &str) -> Result<(), HomeosError> {
    if url.is_empty() {
        return Err(HomeosError::new(
            reasons::VALIDATION_ERROR,
            "URL must not be empty",
        ));
    }

    for c in url.chars() {
        if c.is_control() {
            return Err(HomeosError::new(
                reasons::VALIDATION_ERROR,
                format!("URL '{url}' contains control characters"),
            ));
        }
    }

    let lower = url.to_ascii_lowercase();
    if lower.contains("%00") {
        return Err(HomeosError::new(
            reasons::VALIDATION_ERROR,
            format!("URL '{url}' contains percent-encoded NUL byte ('%00')"),
        ));
    }
    if lower.contains("%2e%2e") {
        return Err(HomeosError::new(
            reasons::VALIDATION_ERROR,
            format!("URL '{url}' contains percent-encoded '..' ('%2e%2e')"),
        ));
    }

    if url.contains('?') {
        return Err(HomeosError::new(
            reasons::VALIDATION_ERROR,
            format!("URL '{url}' must not contain a query string ('?')"),
        ));
    }

    let scheme = match url.split_once("://") {
        Some((s, _)) => s,
        None => return validate_scp_like(url),
    };

    if !ALLOWED_URL_SCHEMES.contains(&scheme) {
        return Err(HomeosError::new(
            reasons::VALIDATION_ERROR,
            format!(
                "URL '{url}' has unsupported scheme '{scheme}'. Allowed: {}",
                ALLOWED_URL_SCHEMES.join(", ")
            ),
        ));
    }

    Ok(())
}

/// Validate the SCP-like `user@host:path` form (no `://`) — the syntax
/// GitHub's SSH clone button produces, e.g. `git@github.com:owner/repo.git`.
///
/// The component charsets deliberately exclude `:`, so the separator is the
/// only colon in the string. Without `://`, git parses anything containing
/// `::` as remote-helper syntax (`<helper>::<address>`, e.g. `ext::`, which
/// executes arbitrary local commands); the structural exclusion is what keeps
/// that injection surface closed. Do NOT relax this to a leading-dash-only
/// check. The host rule is a character alphabet, not a host allowlist — any
/// DNS name, IPv4 address, or ssh-config alias passes (IPv6 literals need the
/// `ssh://` form).
fn validate_scp_like(url: &str) -> Result<(), HomeosError> {
    let neither_form = || {
        HomeosError::new(
            reasons::VALIDATION_ERROR,
            format!(
                "URL '{url}' must be 'scheme://...' (allowed: {}) or SCP-like 'user@host:path'",
                ALLOWED_URL_SCHEMES.join(", ")
            ),
        )
    };

    // An SCP-like attempt has an `@` followed by a later `:`; anything else
    // (bare paths, schemeless host/path strings) matches neither form.
    let Some((user, rest)) = url.split_once('@') else {
        return Err(neither_form());
    };
    let Some((host, path)) = rest.split_once(':') else {
        return Err(neither_form());
    };

    let invalid_component = |component: &str| {
        HomeosError::new(
            reasons::VALIDATION_ERROR,
            format!("URL '{url}' has an invalid {component} in SCP-like form 'user@host:path'"),
        )
    };

    if user.is_empty() || user.starts_with('-') || !user.chars().all(is_scp_user_host_char) {
        return Err(invalid_component("user"));
    }
    if host.is_empty() || host.starts_with('-') || !host.chars().all(is_scp_user_host_char) {
        return Err(invalid_component("host"));
    }
    if path.is_empty() || !path.chars().all(is_scp_path_char) {
        return Err(invalid_component("path"));
    }

    Ok(())
}

fn is_scp_user_host_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
}

fn is_scp_path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '~' | '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name_accepts_lowercase_alphanumeric() {
        // Arrange
        let name = "neovim";

        // Act
        let result = validate_name(name);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_name_accepts_digits_dots_underscores_hyphens() {
        // Arrange
        let names = ["dnf-copr-mise", "a.b.c", "ab_cd", "0name", "rust1", "x"];

        // Act & Assert
        for name in names {
            assert!(validate_name(name).is_ok(), "expected '{name}' to be valid");
        }
    }

    #[test]
    fn test_validate_name_accepts_starting_digit() {
        // Arrange
        let name = "7zip";

        // Act
        let result = validate_name(name);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_name_rejects_empty_string() {
        // Arrange
        let name = "";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("must not be empty"));
    }

    #[test]
    fn test_validate_name_rejects_leading_dash() {
        // Arrange — would be misinterpreted as a flag if passed to a CLI
        let name = "-rf";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("must start with"));
    }

    #[test]
    fn test_validate_name_rejects_leading_dot() {
        // Arrange — hidden-file convention, also the parent-dir prefix
        let name = ".hidden";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("must start with"));
    }

    #[test]
    fn test_validate_name_rejects_leading_underscore() {
        // Arrange — pattern requires alphanumeric start
        let name = "_foo";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_uppercase() {
        // Arrange
        let name = "Foo";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_forward_slash() {
        // Arrange — path traversal via subdirectory
        let name = "foo/bar";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("invalid character"));
    }

    #[test]
    fn test_validate_name_rejects_backslash() {
        // Arrange — Windows path separator
        let name = "foo\\bar";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_whitespace() {
        // Arrange
        let cases = ["foo bar", "foo\tbar", "foo\nbar"];

        // Act & Assert
        for name in cases {
            let err = validate_name(name).unwrap_err();
            assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        }
    }

    #[test]
    fn test_validate_name_rejects_control_characters() {
        // Arrange
        let name = "foo\x01bar";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_nul_byte() {
        // Arrange — terminator on Unix path syscalls
        let name = "foo\0bar";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_double_dot_substring() {
        // Arrange — parent-directory traversal
        let name = "foo..bar";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("'..'"));
    }

    #[test]
    fn test_validate_name_rejects_dot_dot_alone() {
        // Arrange — bare parent-directory reference
        let name = "..";

        // Act
        let result = validate_name(name);

        // Assert
        // Rejected on leading-character rule (first char is '.', not alphanumeric)
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_non_ascii_unicode() {
        // Arrange
        let name = "café";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_percent_encoded() {
        // Arrange — common URL-encoded path traversal payload
        let name = "%2e%2e";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_name_rejects_special_shell_characters() {
        // Arrange — shell metacharacters
        let cases = [
            "foo;bar", "foo|bar", "foo$bar", "foo`bar", "foo&bar", "foo*bar",
        ];

        // Act & Assert
        for name in cases {
            let err = validate_name(name).unwrap_err();
            assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        }
    }

    #[test]
    fn test_validate_name_rejects_trailing_dot_dot() {
        // Arrange
        let name = "foo..";

        // Act
        let result = validate_name(name);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("'..'"));
    }

    #[test]
    fn test_validate_name_accepts_single_character() {
        // Arrange
        let name = "a";

        // Act
        let result = validate_name(name);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_name_accepts_single_digit() {
        // Arrange
        let name = "1";

        // Act
        let result = validate_name(name);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_url_accepts_https() {
        // Arrange
        let url = "https://github.com/homeos-dev/homeos-plugin-dnf";

        // Act
        let result = validate_url(url);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_url_accepts_http() {
        // Arrange
        let url = "http://example.com/repo.git";

        // Act
        let result = validate_url(url);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_url_accepts_git_scheme() {
        // Arrange
        let url = "git://github.com/user/repo.git";

        // Act
        let result = validate_url(url);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_url_accepts_ssh_scheme() {
        // Arrange
        let url = "ssh://git@github.com/user/repo.git";

        // Act
        let result = validate_url(url);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_url_accepts_git_plus_ssh_scheme() {
        // Arrange
        let url = "git+ssh://git@github.com/user/repo.git";

        // Act
        let result = validate_url(url);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_url_rejects_empty() {
        // Arrange
        let url = "";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("must not be empty"));
    }

    #[test]
    fn test_validate_url_rejects_file_scheme() {
        // Arrange — file:// URLs are not in the allowed scheme set
        let url = "file:///etc/passwd";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("scheme 'file'"));
    }

    #[test]
    fn test_validate_url_rejects_javascript_scheme() {
        // Arrange — javascript: would be dangerous in some contexts
        let url = "javascript:alert(1)";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_url_rejects_no_scheme() {
        // Arrange — schemeless host/path string matches neither accepted form
        let url = "github.com/user/repo";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("must be 'scheme://...'"));
        assert!(err.message.contains("SCP-like 'user@host:path'"));
    }

    #[test]
    fn test_validate_url_accepts_scp_like_github_url() {
        // Arrange — the exact form GitHub's SSH clone button copies
        let url = "git@github.com:homeos-dev/homeos-plugin-dnf.git";

        // Act
        let result = validate_url(url);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_url_accepts_scp_like_enterprise_host() {
        // Arrange — GitHub Enterprise-style corporate FQDN
        let url = "git@ghe.example.co.jp:org/repo.git";

        // Act
        let result = validate_url(url);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_url_accepts_scp_like_ssh_config_alias_host() {
        // Arrange — ~/.ssh/config aliases are plain names, not DNS entries
        let url = "git@github-work:org/repo.git";

        // Act
        let result = validate_url(url);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_url_accepts_scp_like_tilde_path() {
        // Arrange — scp-style home-relative path
        let url = "git@host:~/repos/x.git";

        // Act
        let result = validate_url(url);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_url_rejects_remote_helper_syntax() {
        // Arrange — without ://, git parses `<helper>::<address>`; ext::
        // executes arbitrary local commands
        let url = "ext::sh -c payload";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_url_rejects_double_colon_after_scp_shape() {
        // Arrange — the second colon lands in the path component
        let url = "foo@bar::baz";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("invalid path"));
    }

    #[test]
    fn test_validate_url_rejects_scp_like_leading_dash_user() {
        // Arrange — a leading dash could parse as a git/ssh option
        let url = "-user@host:path";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("invalid user"));
    }

    #[test]
    fn test_validate_url_rejects_scp_like_leading_dash_host() {
        // Arrange
        let url = "git@-host:path";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("invalid host"));
    }

    #[test]
    fn test_validate_url_rejects_scp_like_empty_path() {
        // Arrange
        let url = "git@host:";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("invalid path"));
    }

    #[test]
    fn test_validate_url_rejects_windows_drive_path() {
        // Arrange — bare filesystem path; no `@`, so it matches neither form
        let url = "C:\\repos\\x";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("must be 'scheme://...'"));
    }

    #[test]
    fn test_validate_url_rejects_bare_local_path() {
        // Arrange — bare filesystem path
        let url = "/tmp/some-repo";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_url_rejects_control_characters() {
        // Arrange
        let cases = [
            "https://example.com/\x01evil",
            "https://example.com/\nlf",
            "https://example.com/\rcr",
            "https://example.com/\ttab",
        ];

        // Act & Assert
        for url in cases {
            let err = validate_url(url).unwrap_err();
            assert_eq!(err.reason, reasons::VALIDATION_ERROR);
            assert!(
                err.message.contains("control characters"),
                "expected control-chars error for '{url}', got: {}",
                err.message
            );
        }
    }

    #[test]
    fn test_validate_url_rejects_nul_byte() {
        // Arrange — embedded raw NUL terminator
        let url = "https://example.com/\0null";

        // Act
        let result = validate_url(url);

        // Assert — caught by the control-character rule (NUL is control)
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_url_rejects_percent_encoded_nul() {
        // Arrange — %00 in various cases
        let cases = [
            "https://example.com/%00",
            "https://example.com/foo%00bar",
            "https://example.com/PATH%00",
        ];

        // Act & Assert
        for url in cases {
            let err = validate_url(url).unwrap_err();
            assert_eq!(err.reason, reasons::VALIDATION_ERROR);
            assert!(err.message.contains("%00"));
        }
    }

    #[test]
    fn test_validate_url_rejects_percent_encoded_dotdot_lowercase() {
        // Arrange — common URL-encoded path traversal payload
        let url = "https://example.com/%2e%2e/etc/passwd";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("%2e%2e"));
    }

    #[test]
    fn test_validate_url_rejects_percent_encoded_dotdot_uppercase() {
        // Arrange — case-insensitive match
        let url = "https://example.com/%2E%2E/etc";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_url_rejects_percent_encoded_dotdot_mixed_case() {
        // Arrange — mixed case
        let url = "https://example.com/%2e%2E/path";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_url_rejects_query_string() {
        // Arrange — query string carries no meaning for git clone
        let url = "https://example.com/repo.git?evil=1";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
        assert!(err.message.contains("query string"));
    }

    #[test]
    fn test_validate_url_rejects_query_string_in_path_segment() {
        // Arrange — `?` embedded inside what looks like a path segment
        let url = "https://example.com/foo?bar/baz";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_url_accepts_url_with_dot_git_suffix() {
        // Arrange
        let url = "https://github.com/user/repo.git";

        // Act
        let result = validate_url(url);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_url_accepts_url_with_credentials_and_port() {
        // Arrange — well-formed authority components
        let url = "https://user:pass@host.example.com:8443/path";

        // Act
        let result = validate_url(url);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_url_rejects_unknown_scheme_other() {
        // Arrange — a scheme we deliberately don't allow
        let url = "data:text/plain,Hello";

        // Act
        let result = validate_url(url);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, reasons::VALIDATION_ERROR);
    }
}
