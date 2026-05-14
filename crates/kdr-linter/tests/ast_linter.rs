use katana_ast_lint::KatanaAstLint;
use kdr_linter::{KdrLintError, KdrLinter, ViolationReport};
use std::path::{Path, PathBuf};

fn workspace_root() -> Result<PathBuf, KdrLintError> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(crates_dir) = manifest_dir.parent() else {
        return Err(KdrLintError::WorkspaceRoot {
            path: manifest_dir.to_path_buf(),
        });
    };
    let Some(root) = crates_dir.parent() else {
        return Err(KdrLintError::WorkspaceRoot {
            path: crates_dir.to_path_buf(),
        });
    };
    Ok(root.to_path_buf())
}

#[test]
fn ast_linter_workspace_rules() -> Result<(), KdrLintError> {
    KatanaAstLint::from_workspace().assert_clean();
    let root = workspace_root()?;
    let violations = KdrLinter::lint_workspace(&root)?;
    assert!(
        violations.is_empty(),
        "{}",
        ViolationReport::format(&violations)
    );
    Ok(())
}
