use crate::diagnostics::KcfLintError;
use crate::span::SpanOps;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

pub struct SourceFile {
    path: PathBuf,
    source: String,
    syntax: syn::File,
}

impl SourceFile {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn syntax(&self) -> &syn::File {
        &self.syntax
    }

    pub fn is_under(&self, root: &Path) -> bool {
        self.path.starts_with(root)
    }
}

pub struct WorkspaceModel {
    root: PathBuf,
    rust_files: Vec<SourceFile>,
}

impl WorkspaceModel {
    pub fn load(root: &Path) -> Result<Self, KcfLintError> {
        let mut rust_files = Vec::new();
        for target in Self::target_roots(root) {
            rust_files.extend(Self::collect_rs_files(&target)?);
        }
        Ok(Self {
            root: root.to_path_buf(),
            rust_files,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn rust_files(&self) -> &[SourceFile] {
        &self.rust_files
    }

    fn target_roots(root: &Path) -> Vec<PathBuf> {
        vec![
            root.join("crates/kcf-linter/src"),
            root.join("crates/kcf-linter/tests"),
            root.join("crates/katana-canvas-forge/src"),
            root.join("crates/katana-canvas-forge-cli/src"),
        ]
    }

    fn collect_rs_files(root: &Path) -> Result<Vec<SourceFile>, KcfLintError> {
        let mut files = Vec::new();
        if !root.exists() {
            return Ok(files);
        }
        for entry in WalkBuilder::new(root).standard_filters(true).build() {
            let entry = entry.map_err(|source| KcfLintError::Walk {
                path: root.to_path_buf(),
                source,
            })?;
            let path = entry.path();
            if Self::is_rust_file(path) {
                files.push(Self::parse_file(path)?);
            }
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(files)
    }

    fn is_rust_file(path: &Path) -> bool {
        path.is_file() && path.extension().is_some_and(|extension| extension == "rs")
    }

    fn parse_file(path: &Path) -> Result<SourceFile, KcfLintError> {
        let source = std::fs::read_to_string(path).map_err(|source| KcfLintError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let syntax = syn::parse_file(&source).map_err(|source| {
            let location = SpanOps::start(source.span());
            KcfLintError::RustParse {
                path: path.to_path_buf(),
                line: location.line,
                column: location.column,
                message: source.to_string(),
            }
        })?;
        Ok(SourceFile {
            path: path.to_path_buf(),
            source,
            syntax,
        })
    }
}
