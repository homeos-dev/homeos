use crate::config::Config;
use crate::context::Context;
use crate::error::{HomeosError, reasons};
use crate::git;
use std::fs;

pub fn run(
    ctx: &Context,
    url: Option<&str>,
    strip_git: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = ctx.data_dir();
    let config_path = ctx.config_path();

    if config_path.exists() {
        return Err(HomeosError::new(
            reasons::ALREADY_EXISTS,
            format!("Already initialized at {}", data_dir.display()),
        )
        .into());
    }

    let data_dir_is_non_empty = data_dir
        .read_dir()
        .map(|mut iter| iter.next().is_some())
        .unwrap_or(false);
    if data_dir_is_non_empty {
        return Err(HomeosError::new(
            reasons::DATA_DIR_NOT_EMPTY,
            format!("Data directory at {} is not empty", data_dir.display()),
        )
        .into());
    }

    if let Some(url) = url {
        if let Some(parent) = data_dir.parent() {
            fs::create_dir_all(parent)?;
        }

        git::clone(url, data_dir)?;

        if !config_path.exists() {
            fs::remove_dir_all(data_dir)?;
            return Err(HomeosError::new(
                reasons::NOT_A_VALID_HOMEOS_REPO,
                "Not a valid homeos repository. Cloned directory removed.",
            )
            .into());
        }

        if strip_git {
            let git_dir = data_dir.join(".git");
            if git_dir.exists() {
                fs::remove_dir_all(&git_dir)?;
            }
        }

        crate::commands::update_check::seed_cache(data_dir)?;

        println!(
            "Initialized homeos at {} (cloned from {})",
            data_dir.display(),
            url
        );
    } else {
        let packages_dir = ctx.packages_dir();
        fs::create_dir_all(&packages_dir)?;

        let plugins_dir = ctx.plugins_dir();
        fs::create_dir_all(&plugins_dir)?;

        let config = Config::default();
        config.save(&config_path)?;

        let gitignore_path = ctx.gitignore_path();
        if !gitignore_path.exists() {
            fs::write(&gitignore_path, "state.yml\n.last-update-check\n")?;
        }

        crate::commands::update_check::seed_cache(data_dir)?;

        git::init(data_dir)?;

        println!("Initialized homeos at {}", data_dir.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{HomeosError, reasons};
    use std::process::Command;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, Context) {
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().join("homeos")));
        (tmp, ctx)
    }

    fn create_local_git_repo(dir: &std::path::Path) {
        fs::create_dir_all(dir).unwrap();
        Command::new("git")
            .args(["init", &dir.to_string_lossy()])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &dir.to_string_lossy(),
                "commit",
                "--allow-empty",
                "-m",
                "init",
            ])
            .output()
            .unwrap();
    }

    fn create_source_repo_with_config(source_dir: &std::path::Path) {
        create_local_git_repo(source_dir);
        fs::write(source_dir.join("homeos.yml"), "packages: {}\n").unwrap();
        Command::new("git")
            .args(["-C", &source_dir.to_string_lossy(), "add", "homeos.yml"])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &source_dir.to_string_lossy(),
                "commit",
                "-m",
                "add config",
            ])
            .output()
            .unwrap();
    }

    #[test]
    fn test_init_creates_structure() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert
        assert!(ctx.data_dir().exists());
        assert!(ctx.packages_dir().exists());
        assert!(ctx.plugins_dir().exists());
        assert!(ctx.config_path().exists());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.packages.is_empty());
    }

    #[test]
    fn test_init_already_initialized_returns_error() {
        // Arrange
        let (_tmp, ctx) = fixture();
        run(&ctx, None, false).unwrap();
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config
            .packages
            .insert("test".to_string(), crate::config::PackageConfig::default());
        config.save(&ctx.config_path()).unwrap();

        // Act
        let result = run(&ctx, None, false);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.starts_with("Already initialized at "),
            "unexpected error: {}",
            err
        );
        // Config should be preserved (not overwritten)
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.packages.len(), 1);
    }

    #[test]
    fn test_init_flat_directory_paths() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert
        let data_dir = ctx.data_dir();
        assert!(data_dir.join("packages").exists());
        assert!(data_dir.join("plugins").exists());
        assert!(data_dir.join("homeos.yml").exists());
        assert!(data_dir.join(".gitignore").exists());
        // The old repos/default/ segment must not exist anywhere under data_dir.
        assert!(!data_dir.join("repos").exists());
    }

    #[test]
    fn test_init_creates_plugins_directory() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert
        assert!(ctx.plugins_dir().exists());
        assert!(ctx.plugins_dir().is_dir());
    }

    #[test]
    fn test_init_creates_gitignore_excluding_state_yml_and_update_check() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert
        let gitignore_path = ctx.gitignore_path();
        assert!(gitignore_path.exists());
        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(content, "state.yml\n.last-update-check\n");
    }

    #[test]
    fn test_init_already_initialized_preserves_gitignore() {
        // Arrange
        let (_tmp, ctx) = fixture();
        run(&ctx, None, false).unwrap();
        fs::write(ctx.gitignore_path(), "state.yml\ncustom\n").unwrap();

        // Act
        let result = run(&ctx, None, false);

        // Assert
        assert!(result.is_err());
        let content = fs::read_to_string(ctx.gitignore_path()).unwrap();
        assert_eq!(content, "state.yml\ncustom\n");
    }

    #[test]
    fn test_init_scaffold_errors_if_data_dir_not_empty() {
        // Arrange
        let (_tmp, ctx) = fixture();
        fs::create_dir_all(ctx.data_dir()).unwrap();
        fs::write(ctx.data_dir().join("stray.txt"), "preexisting\n").unwrap();

        // Act
        let result = run(&ctx, None, false);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.starts_with("Data directory at ") && err.ends_with(" is not empty"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn test_init_scaffold_succeeds_if_data_dir_exists_but_empty() {
        // Arrange
        let (_tmp, ctx) = fixture();
        fs::create_dir_all(ctx.data_dir()).unwrap();

        // Act
        let result = run(&ctx, None, false);

        // Assert
        assert!(result.is_ok());
        assert!(ctx.config_path().exists());
    }

    #[test]
    fn test_init_with_url_clones_repo() {
        // Arrange
        let (_tmp, ctx) = fixture();
        let source_dir = TempDir::new().unwrap();
        create_source_repo_with_config(source_dir.path());

        // Act
        run(&ctx, Some(&source_dir.path().to_string_lossy()), false).unwrap();

        // Assert
        assert!(ctx.data_dir().exists());
        assert!(ctx.config_path().exists());
        assert!(ctx.data_dir().join(".git").exists());
    }

    #[test]
    fn test_init_with_url_errors_if_already_initialized() {
        // Arrange
        let (_tmp, ctx) = fixture();
        run(&ctx, None, false).unwrap();

        // Act
        let source_dir = TempDir::new().unwrap();
        create_local_git_repo(source_dir.path());
        let result = run(&ctx, Some(&source_dir.path().to_string_lossy()), false);

        // Assert — returns error, original scaffold preserved
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.starts_with("Already initialized at "),
            "unexpected error: {}",
            err
        );
        assert!(ctx.packages_dir().exists());
    }

    #[test]
    fn test_init_with_url_errors_if_data_dir_not_empty() {
        // Arrange
        let (_tmp, ctx) = fixture();
        fs::create_dir_all(ctx.data_dir()).unwrap();
        fs::write(ctx.data_dir().join("stray.txt"), "preexisting\n").unwrap();
        let source_dir = TempDir::new().unwrap();
        create_source_repo_with_config(source_dir.path());

        // Act
        let result = run(&ctx, Some(&source_dir.path().to_string_lossy()), false);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.starts_with("Data directory at ") && err.ends_with(" is not empty"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn test_init_with_url_invalid_url() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        let result = run(&ctx, Some("not-a-valid-url"), false);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.starts_with("git clone failed:"));
    }

    #[test]
    fn test_init_strip_git_removes_git_directory() {
        // Arrange
        let (_tmp, ctx) = fixture();
        let source_dir = TempDir::new().unwrap();
        create_source_repo_with_config(source_dir.path());

        // Act
        run(&ctx, Some(&source_dir.path().to_string_lossy()), true).unwrap();

        // Assert
        assert!(ctx.data_dir().exists());
        assert!(ctx.config_path().exists());
        assert!(!ctx.data_dir().join(".git").exists());
    }

    #[test]
    fn test_init_strip_git_false_preserves_git_directory() {
        // Arrange
        let (_tmp, ctx) = fixture();
        let source_dir = TempDir::new().unwrap();
        create_source_repo_with_config(source_dir.path());

        // Act
        run(&ctx, Some(&source_dir.path().to_string_lossy()), false).unwrap();

        // Assert
        assert!(ctx.data_dir().join(".git").exists());
    }

    #[test]
    fn test_init_strip_git_without_url_is_noop() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx, None, true).unwrap();

        // Assert — scaffold mode ignores strip_git
        assert!(ctx.data_dir().exists());
        assert!(ctx.config_path().exists());
    }

    #[test]
    fn test_init_already_initialized_reason_is_already_exists() {
        // Arrange
        let (_tmp, ctx) = fixture();
        run(&ctx, None, false).unwrap();

        // Act
        let result = run(&ctx, None, false);

        // Assert
        let err = result.unwrap_err();
        let homeos_err = err
            .downcast_ref::<HomeosError>()
            .expect("expected HomeosError");
        assert_eq!(homeos_err.reason, reasons::ALREADY_EXISTS);
    }

    #[test]
    fn test_init_data_dir_not_empty_reason_is_data_dir_not_empty() {
        // Arrange
        let (_tmp, ctx) = fixture();
        fs::create_dir_all(ctx.data_dir()).unwrap();
        fs::write(ctx.data_dir().join("stray.txt"), "preexisting\n").unwrap();

        // Act
        let result = run(&ctx, None, false);

        // Assert
        let err = result.unwrap_err();
        let homeos_err = err
            .downcast_ref::<HomeosError>()
            .expect("expected HomeosError");
        assert_eq!(homeos_err.reason, reasons::DATA_DIR_NOT_EMPTY);
    }

    #[test]
    fn test_init_with_url_invalid_url_reason_is_git_clone_failed() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        let result = run(&ctx, Some("not-a-valid-url"), false);

        // Assert
        let err = result.unwrap_err();
        let homeos_err = err
            .downcast_ref::<HomeosError>()
            .expect("expected HomeosError");
        assert_eq!(homeos_err.reason, reasons::GIT_CLONE_FAILED);
    }

    #[test]
    fn test_init_with_url_rejects_repo_without_homeos_yml_reason() {
        // Arrange
        let (_tmp, ctx) = fixture();
        let source_dir = TempDir::new().unwrap();
        create_local_git_repo(source_dir.path());

        // Act
        let result = run(&ctx, Some(&source_dir.path().to_string_lossy()), false);

        // Assert
        let err = result.unwrap_err();
        let homeos_err = err
            .downcast_ref::<HomeosError>()
            .expect("expected HomeosError");
        assert_eq!(homeos_err.reason, reasons::NOT_A_VALID_HOMEOS_REPO);
    }

    #[test]
    fn test_init_scaffold_initializes_git_repo() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert — scaffold mode creates a .git directory so the data dir is
        // a git repository from the moment it exists.
        let git_dir = ctx.data_dir().join(".git");
        assert!(git_dir.exists());
        assert!(git_dir.is_dir());
    }

    #[test]
    fn test_init_scaffold_uses_main_as_initial_branch() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert — HEAD must point at refs/heads/main so the data directory
        // has a consistent branch name across machines, regardless of the
        // user's `init.defaultBranch` config.
        let output = Command::new("git")
            .args([
                "-C",
                &ctx.data_dir().to_string_lossy(),
                "symbolic-ref",
                "--short",
                "HEAD",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "symbolic-ref failed in data dir");
        let head = String::from_utf8(output.stdout).unwrap();
        assert_eq!(head.trim(), "main");
    }

    #[test]
    fn test_init_scaffold_git_repo_tracks_scaffolded_files() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert — `git status` succeeds against the data dir, confirming
        // the directory is a valid working tree and the scaffolded files
        // are visible to git (homeos.yml is tracked; state.yml and the
        // update-check cache are excluded via .gitignore).
        let output = Command::new("git")
            .args([
                "-C",
                &ctx.data_dir().to_string_lossy(),
                "status",
                "--porcelain",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "git status failed in data dir");
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.contains("homeos.yml"),
            "expected homeos.yml in `git status`, got: {stdout}"
        );
        assert!(
            stdout.contains(".gitignore"),
            "expected .gitignore in `git status`, got: {stdout}"
        );
        assert!(
            !stdout.contains("state.yml"),
            "state.yml should be ignored, got: {stdout}"
        );
    }

    #[test]
    fn test_init_scaffold_does_not_create_commits() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert — homeos init must NOT create commits; the agent walks the
        // user through the initial commit per the `homeos agents-md`
        // "First-time setup" guide. `git rev-list HEAD` errors when no
        // commits exist.
        let output = Command::new("git")
            .args([
                "-C",
                &ctx.data_dir().to_string_lossy(),
                "rev-list",
                "--count",
                "HEAD",
            ])
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "expected no commits, but rev-list succeeded with stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }

    #[test]
    fn test_init_scaffold_seeds_update_check_cache() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert — `.last-update-check` exists, parses, and carries the current
        // binary's tag so the user is not pinged within the first 7-day window.
        let cache_path = crate::commands::update_check::cache_path(ctx.data_dir());
        assert!(cache_path.exists(), "expected .last-update-check to exist");
        let cache: crate::commands::update_check::UpdateCheckCache =
            serde_json::from_str(&fs::read_to_string(&cache_path).unwrap()).unwrap();
        assert_eq!(
            cache.latest_tag,
            crate::commands::update_check::current_tag()
        );
        assert!(cache.last_checked_at > 0);
    }

    #[test]
    fn test_init_with_url_seeds_update_check_cache() {
        // Arrange
        let (_tmp, ctx) = fixture();
        let source_dir = TempDir::new().unwrap();
        create_source_repo_with_config(source_dir.path());

        // Act
        run(&ctx, Some(&source_dir.path().to_string_lossy()), false).unwrap();

        // Assert
        let cache_path = crate::commands::update_check::cache_path(ctx.data_dir());
        assert!(cache_path.exists());
        let cache: crate::commands::update_check::UpdateCheckCache =
            serde_json::from_str(&fs::read_to_string(&cache_path).unwrap()).unwrap();
        assert_eq!(
            cache.latest_tag,
            crate::commands::update_check::current_tag()
        );
    }

    #[test]
    fn test_init_gitignore_excludes_last_update_check() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        run(&ctx, None, false).unwrap();

        // Assert
        let content = fs::read_to_string(ctx.gitignore_path()).unwrap();
        assert!(content.contains(".last-update-check"));
    }

    #[test]
    fn test_init_with_url_rejects_repo_without_homeos_yml() {
        // Arrange
        let (_tmp, ctx) = fixture();
        let source_dir = TempDir::new().unwrap();
        create_local_git_repo(source_dir.path());

        // Act
        let result = run(&ctx, Some(&source_dir.path().to_string_lossy()), false);

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Not a valid homeos repository. Cloned directory removed."
        );
        assert!(!ctx.data_dir().exists());
    }
}
