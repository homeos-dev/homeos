use crate::commands::detect_shell;
use crate::context::Context;
use crate::error::{HomeosError, reasons};
use crate::output::OutputFormat;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

/// Resolve and validate the target directory for `homeos cd`.
/// Returns the data directory path if it exists, or an error if not.
pub fn resolve_target(ctx: &Context) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = ctx.data_dir().to_path_buf();
    if !dir.exists() {
        return Err(HomeosError::new(
            reasons::DATA_DIR_NOT_FOUND,
            format!(
                "Data directory not found at {}. Run 'homeos init' first.",
                dir.display()
            ),
        )
        .into());
    }
    Ok(dir)
}

pub fn run(ctx: &Context, print: bool) -> Result<(), Box<dyn std::error::Error>> {
    let dir = resolve_target(ctx)?;
    if print {
        return print_path(&dir, ctx.output_format(), &mut std::io::stdout());
    }
    let shell = detect_shell();

    let status = Command::new(&shell).current_dir(&dir).status()?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn print_path<W: Write>(
    dir: &std::path::Path,
    format: OutputFormat,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Json => {
            let value = serde_json::json!({ "path": dir.display().to_string() });
            writeln!(writer, "{value}")?;
        }
        OutputFormat::Text => {
            writeln!(writer, "{}", dir.display())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, Context) {
        let tmp = TempDir::new().unwrap();
        let ctx = Context::new(Some(tmp.path().join("homeos")));
        (tmp, ctx)
    }

    #[test]
    fn test_resolve_target_returns_data_dir() {
        // Arrange
        let (_tmp, ctx) = fixture();
        init::run(&ctx, None, false).unwrap();

        // Act
        let result = resolve_target(&ctx).unwrap();

        // Assert
        assert_eq!(result, ctx.data_dir());
    }

    #[test]
    fn test_resolve_target_dir_exists() {
        // Arrange
        let (_tmp, ctx) = fixture();
        init::run(&ctx, None, false).unwrap();

        // Act
        let result = resolve_target(&ctx).unwrap();

        // Assert
        assert!(result.exists());
        assert!(result.is_dir());
    }

    #[test]
    fn test_resolve_target_errors_when_not_initialized() {
        // Arrange
        let (_tmp, ctx) = fixture();

        // Act
        let result = resolve_target(&ctx);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Data directory not found"));
        assert!(err.contains("homeos init"));
    }

    #[test]
    fn test_print_path_text_emits_single_line() {
        // Arrange
        let dir = std::path::PathBuf::from("/home/user/.local/share/homeos");
        let mut output = Vec::new();

        // Act
        print_path(&dir, OutputFormat::Text, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text, "/home/user/.local/share/homeos\n");
    }

    #[test]
    fn test_print_path_json_emits_object_with_path() {
        // Arrange
        let dir = std::path::PathBuf::from("/home/user/.local/share/homeos");
        let mut output = Vec::new();

        // Act
        print_path(&dir, OutputFormat::Json, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["path"], "/home/user/.local/share/homeos");
    }

    #[test]
    fn test_run_print_emits_initialized_data_dir_to_writer() {
        // Arrange
        let (_tmp, ctx) = fixture();
        init::run(&ctx, None, false).unwrap();
        let dir = resolve_target(&ctx).unwrap();
        let mut output = Vec::new();

        // Act
        print_path(&dir, OutputFormat::Text, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert_eq!(
            text.trim_end_matches('\n'),
            ctx.data_dir().display().to_string()
        );
    }

    #[test]
    fn test_run_print_errors_when_data_dir_missing() {
        // Arrange — no init was run, so the data directory does not exist
        let (_tmp, ctx) = fixture();

        // Act
        let result = run(&ctx, true);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err();
        let homeos_err = err
            .downcast_ref::<HomeosError>()
            .expect("expected HomeosError");
        assert_eq!(homeos_err.reason, reasons::DATA_DIR_NOT_FOUND);
    }
}
