use kcf_linter::{KcfLintError, KcfLinter, ViolationReport};
use std::path::{Path, PathBuf};

fn workspace_root() -> Result<PathBuf, KcfLintError> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(crates_dir) = manifest_dir.parent() else {
        return Err(KcfLintError::WorkspaceRoot {
            path: manifest_dir.to_path_buf(),
        });
    };
    let Some(root) = crates_dir.parent() else {
        return Err(KcfLintError::WorkspaceRoot {
            path: crates_dir.to_path_buf(),
        });
    };
    Ok(root.to_path_buf())
}

#[test]
fn ast_linter_workspace_rules() -> Result<(), KcfLintError> {
    let root = workspace_root()?;
    let violations = KcfLinter::lint_workspace(&root)?;
    assert!(
        violations.is_empty(),
        "{}",
        ViolationReport::format(&violations)
    );
    Ok(())
}
