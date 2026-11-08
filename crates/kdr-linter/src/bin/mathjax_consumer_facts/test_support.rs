use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy)]
enum Inspector {
    MathJax,
    NativeWidth,
    MermaidClassic,
}

pub(super) struct Fixture {
    root: PathBuf,
    inspector: Inspector,
}

impl Fixture {
    pub(super) fn copy_mathjax_inputs(
        input_paths: &[&str],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::copy_inputs(
            "mathjax-default-static-consumer-closure-fix",
            input_paths,
            Inspector::MathJax,
        )
    }

    pub(super) fn inspect_parser_pin(&self) -> Result<String, String> {
        let workspace =
            fs::read_to_string(self.root.join("Cargo.toml")).map_err(|error| error.to_string())?;
        let linter = fs::read_to_string(self.root.join("crates/kdr-linter/Cargo.toml"))
            .map_err(|error| error.to_string())?;
        let lock =
            fs::read_to_string(self.root.join("Cargo.lock")).map_err(|error| error.to_string())?;
        super::source::parser_pin(&self.root, &lock, &workspace, &linter)
    }

    pub(super) fn inspect_parser_pin_inputs(
        &self,
        workspace: &str,
        linter: &str,
        lock: &str,
    ) -> Result<String, String> {
        fs::write(self.root.join("Cargo.toml"), workspace).map_err(|error| error.to_string())?;
        fs::write(self.root.join("crates/kdr-linter/Cargo.toml"), linter)
            .map_err(|error| error.to_string())?;
        fs::write(self.root.join("Cargo.lock"), lock).map_err(|error| error.to_string())?;
        super::source::parser_pin(&self.root, lock, workspace, linter)
    }

    pub(super) fn copy_native_width_inputs(
        input_paths: &[&str],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::copy_inputs(
            "native-width-hash-route-fix-r1",
            input_paths,
            Inspector::NativeWidth,
        )
    }

    fn copy_inputs(
        namespace: &str,
        input_paths: &[&str],
        inspector: Inspector,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let repository = repository_root()?;
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = repository
            .join("tmp/quality")
            .join(namespace)
            .join("fixtures")
            .join(format!("{}-{id}", std::process::id()));
        Self::copy_inputs_at(&repository, root, input_paths, inspector)
    }

    fn copy_inputs_at(
        repository: &Path,
        root: PathBuf,
        input_paths: &[&str],
        inspector: Inspector,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        fs::create_dir_all(root.parent().ok_or("fixture parent")?)?;
        fs::create_dir(&root)?;
        let fixture = Self { root, inspector };
        for relative in input_paths {
            let source = repository.join(relative);
            let target = fixture.root.join(relative);
            fs::create_dir_all(target.parent().ok_or("fixture parent")?)?;
            fs::copy(source, target)?;
        }
        Ok(fixture)
    }

    pub(super) fn copy_mermaid_classic_inputs(
        input_paths: &[&str],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::copy_inputs(
            "rust-classic-script-consumer-facts-implementation-r1",
            input_paths,
            Inspector::MermaidClassic,
        )
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn reject_unique_mutations(
        &self,
        relative: &str,
        cases: &[(&str, &str, &str)],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.inspect()?;
        for (before, after, expected) in cases {
            let source = fs::read_to_string(self.root.join(relative))?;
            super::test_support_verdict_tests::assert_unique_anchor(&source, before)?;
            self.replace(relative, before, after)?;
            self.reject(expected)?;
            self.restore(relative)?;
            self.inspect()?;
        }
        Ok(())
    }

    pub(super) fn replace(
        &self,
        relative: &str,
        original: &str,
        replacement: &str,
    ) -> std::io::Result<()> {
        let target = self.root.join(relative);
        let content = fs::read_to_string(&target)?;
        assert!(
            content.contains(original),
            "fixture source changed: {relative}"
        );
        fs::write(&target, content.replacen(original, replacement, 1))
    }

    pub(super) fn restore(&self, relative: &str) -> Result<(), Box<dyn std::error::Error>> {
        fs::copy(repository_root()?.join(relative), self.root.join(relative))?;
        Ok(())
    }

    pub(super) fn restore_from(&self, relative: &str, source: &Path) -> std::io::Result<u64> {
        fs::copy(source, self.root.join(relative))
    }

    pub(super) fn reject(&self, expected: &str) -> Result<(), Box<dyn std::error::Error>> {
        let result = self.inspect();
        let accepted_message = match self.inspector {
            Inspector::MathJax => "mutated route was accepted",
            Inspector::NativeWidth => "mutated NativeWidth hash route was accepted",
            Inspector::MermaidClassic => "mutated Mermaid classic route was accepted",
        };
        super::test_support_verdict_tests::assert_rejected(result, expected, accepted_message)
    }

    fn inspect(&self) -> Result<Value, String> {
        match self.inspector {
            Inspector::MathJax => super::inspect_root(&self.root),
            Inspector::NativeWidth => super::native_width_hash::inspect_root(&self.root),
            Inspector::MermaidClassic => super::mermaid_classic_scripts::inspect_root(&self.root),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub(super) fn repository_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("repository root")?
        .to_path_buf())
}

#[cfg(test)]
#[path = "test_support_io_tests.rs"]
mod io_tests;
