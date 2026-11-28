use katana_ast_lint::{KatanaAstLint, Violation, config::KalConfig};
use std::path::Path;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[test]
fn ast_linter_counts_product_lines_after_external_test_module() -> TestResult<()> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let boundary = root.join("boundary.rs");
    let overflow = root.join("overflow.rs");
    std::fs::write(&boundary, file_length_source(199))?;
    std::fs::write(&overflow, file_length_source(200))?;
    let violations = lint_fixture(root);
    let length_errors = violations
        .iter()
        .filter(|violation| violation.message.starts_with("File exceeds 200-line limit"))
        .collect::<Vec<_>>();

    assert_eq!(length_errors.len(), 1, "{violations:?}");
    assert_ne!(length_errors[0].file, boundary);
    assert_eq!(length_errors[0].file, overflow);
    assert_eq!((length_errors[0].line, length_errors[0].column), (1, 1));
    assert!(length_errors[0].message.contains("current: 201"));
    Ok(())
}

#[test]
fn ast_linter_counts_function_open_and_close_lines() -> TestResult<()> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let source = format!(
        "{}\n{}",
        function_source("within_limit", 29),
        function_source("over_limit", 30),
    );
    let path = root.join("functions.rs");
    std::fs::write(&path, source)?;
    let violations = lint_fixture(root);
    let length_errors = violations
        .iter()
        .filter(|violation| violation.message.contains("exceeds 30-line limit"))
        .collect::<Vec<_>>();

    assert_eq!(length_errors.len(), 1, "{violations:?}");
    assert_eq!(length_errors[0].file, path);
    assert_eq!((length_errors[0].line, length_errors[0].column), (33, 4));
    assert!(length_errors[0].message.contains("Function `over_limit`"));
    assert!(length_errors[0].message.contains("current: 31"));
    Ok(())
}

fn lint_fixture(root: &Path) -> Vec<Violation> {
    let config = KalConfig {
        source_roots: vec![root.to_path_buf()],
        ..KalConfig::default()
    };
    KatanaAstLint::with_config(config).violations()
}

fn file_length_source(after_module: usize) -> String {
    let mut source =
        String::from("fn before_module() {}\n#[cfg(test)]\n#[path = \"side.rs\"]\nmod side;\n");
    for index in 0..after_module {
        source.push_str(&format!("fn after_module_{index}() {{}}\n"));
    }
    source
}

fn function_source(name: &str, statements: usize) -> String {
    format!(
        "fn {name}() {{\n{}}}\n",
        "    let _ = ();\n".repeat(statements)
    )
}
