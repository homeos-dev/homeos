pub mod cd;
pub mod completion;
pub mod guide;
pub mod init;
pub mod package;
pub mod plugin;
pub mod update_check;

use std::ffi::OsStr;

/// Detect the user's shell for spawning interactive subshells.
/// Uses `SHELL` env var if set, otherwise falls back to the platform default:
/// `/bin/sh` on Unix, or `pwsh` (PowerShell 7+) on Windows when available on
/// `PATH`, otherwise `powershell` (Windows PowerShell 5.1).
pub fn detect_shell() -> String {
    resolve_shell(std::env::var("SHELL").ok())
}

fn resolve_shell(shell_env: Option<String>) -> String {
    resolve_shell_with(shell_env, cfg!(windows), pwsh_on_path())
}

fn resolve_shell_with(shell_env: Option<String>, is_windows: bool, pwsh_available: bool) -> String {
    shell_env.unwrap_or_else(|| {
        if is_windows {
            windows_shell_for(pwsh_available).to_string()
        } else {
            "/bin/sh".to_string()
        }
    })
}

/// Selects the Windows shell binary given whether `pwsh` is available on `PATH`.
/// `pwsh` (PowerShell 7+) is preferred; `powershell` (Windows PowerShell 5.1,
/// preinstalled on every modern Windows install) is the fallback. This is the
/// single source of truth for Windows shell selection, shared by both
/// shell-spawning contexts: action-script execution and the cd-family subshell.
pub(crate) fn windows_shell_for(pwsh_available: bool) -> &'static str {
    if pwsh_available { "pwsh" } else { "powershell" }
}

/// Returns `true` when `pwsh` (PowerShell 7+) is discoverable on `PATH`.
pub(crate) fn pwsh_on_path() -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    pwsh_on_path_in(&path)
}

fn pwsh_on_path_in(path: &OsStr) -> bool {
    for dir in std::env::split_paths(path) {
        for name in ["pwsh", "pwsh.exe"] {
            if dir.join(name).is_file() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_shell_uses_env_var_when_set() {
        // Arrange
        let shell_env = Some("/usr/bin/fish".to_string());

        // Act
        let shell = resolve_shell(shell_env);

        // Assert
        assert_eq!(shell, "/usr/bin/fish");
    }

    #[test]
    fn test_resolve_shell_with_uses_env_var_regardless_of_platform() {
        // Arrange — env var set; a Windows host without pwsh would otherwise pick powershell
        let shell_env = Some("/usr/bin/fish".to_string());

        // Act
        let shell = resolve_shell_with(shell_env, true, false);

        // Assert
        assert_eq!(shell, "/usr/bin/fish");
    }

    #[test]
    fn test_resolve_shell_with_falls_back_to_sh_on_unix() {
        // Arrange
        let shell_env = None;

        // Act
        let shell = resolve_shell_with(shell_env, false, false);

        // Assert
        assert_eq!(shell, "/bin/sh");
    }

    #[test]
    fn test_resolve_shell_with_prefers_pwsh_on_windows_when_present() {
        // Arrange
        let shell_env = None;

        // Act
        let shell = resolve_shell_with(shell_env, true, true);

        // Assert
        assert_eq!(shell, "pwsh");
    }

    #[test]
    fn test_resolve_shell_with_falls_back_to_powershell_on_windows_when_pwsh_absent() {
        // Arrange
        let shell_env = None;

        // Act
        let shell = resolve_shell_with(shell_env, true, false);

        // Assert
        assert_eq!(shell, "powershell");
    }

    #[test]
    fn test_pwsh_on_path_in_returns_true_when_pwsh_is_present() {
        // Arrange
        let dir = tempfile::tempdir().unwrap();
        let exe_name = if cfg!(windows) { "pwsh.exe" } else { "pwsh" };
        let pwsh_path = dir.path().join(exe_name);
        std::fs::write(&pwsh_path, "").unwrap();
        let path_var = dir.path().as_os_str().to_owned();

        // Act
        let found = pwsh_on_path_in(&path_var);

        // Assert
        assert!(found);
    }

    #[test]
    fn test_pwsh_on_path_in_returns_false_when_pwsh_is_absent() {
        // Arrange
        let dir = tempfile::tempdir().unwrap();
        let path_var = dir.path().as_os_str().to_owned();

        // Act
        let found = pwsh_on_path_in(&path_var);

        // Assert
        assert!(!found);
    }
}
