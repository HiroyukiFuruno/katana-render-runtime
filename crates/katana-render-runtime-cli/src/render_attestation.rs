use super::DiagramInputFormat;
use anyhow::{Context, ensure};
use katana_render_runtime::{RenderInput, RenderOutput};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[path = "render_attestation_classic.rs"]
mod classic;
#[path = "render_attestation_files.rs"]
mod files;
#[path = "render_attestation_receipt.rs"]
mod receipt;

use files::{
    FileRecord, create_artifacts, input_snapshots, sha256, validate_destination, write_new,
};

pub(super) struct AttestationPaths<'a> {
    pub(super) input: &'a Path,
    pub(super) output: Option<&'a Path>,
    pub(super) runtime: &'a Path,
    pub(super) receipt: &'a Path,
}

impl<'a> AttestationPaths<'a> {
    fn validate(&self) -> anyhow::Result<&'a Path> {
        let output = self
            .output
            .context("render attestation requires an output file")?;
        validate_destination(self.receipt)?;
        validate_destination(output)?;
        ensure!(
            self.receipt != output,
            "output and attestation paths must differ"
        );
        Ok(output)
    }
}

fn validate_token(token: Option<&str>) -> anyhow::Result<&str> {
    let token = token.context("render attestation requires an attempt token")?;
    ensure!(
        !token.is_empty()
            && token.len() <= 128
            && token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')),
        "invalid attestation attempt token"
    );
    Ok(token)
}

pub(super) struct RenderAttestation {
    receipt: PathBuf,
    output: PathBuf,
    artifacts: PathBuf,
    token: String,
    format: DiagramInputFormat,
    input_before: FileRecord,
    runtime_before: FileRecord,
    executable_before: FileRecord,
    classic: Option<classic::ClassicCapture>,
}

impl RenderAttestation {
    pub(super) fn begin(
        paths: AttestationPaths<'_>,
        token: Option<&str>,
        format: DiagramInputFormat,
        raw_source: &[u8],
    ) -> anyhow::Result<Self> {
        let token = validate_token(token)?;
        let output = paths.validate()?;
        let input_before = FileRecord::read(paths.input)?;
        ensure!(
            input_before.sha256 == sha256(raw_source)
                && input_before.size == u64::try_from(raw_source.len())?,
            "input changed after the CLI read"
        );
        let runtime_before = FileRecord::read(paths.runtime)?;
        let executable_before = FileRecord::read(&std::env::current_exe()?)?;
        let artifacts = create_artifacts(paths.receipt, raw_source)?;
        Ok(Self {
            receipt: paths.receipt.to_path_buf(),
            output: output.to_path_buf(),
            artifacts,
            token: token.to_owned(),
            format,
            input_before,
            runtime_before,
            executable_before,
            classic: None,
        })
    }

    pub(super) fn capture_input(&self, input: &RenderInput) -> anyhow::Result<()> {
        write_new(
            &self.artifacts.join("request-source.mmd"),
            input.source.as_bytes(),
        )?;
        write_new(
            &self.artifacts.join("render-input.json"),
            &serde_json::to_vec(input)?,
        )
    }

    pub(super) fn capture_render(
        &mut self,
        source: &str,
        render: impl FnOnce() -> anyhow::Result<RenderOutput>,
    ) -> anyhow::Result<RenderOutput> {
        ensure!(
            self.classic.is_none(),
            "classic render was already captured"
        );
        let (output, capture) = classic::capture(&self.artifacts, source, render)?;
        self.classic = Some(capture);
        Ok(output)
    }

    pub(super) fn complete(self, input: &RenderInput, output: &RenderOutput) -> anyhow::Result<()> {
        ensure!(
            output.diagnostics.errors.is_empty() && output.diagnostics.warnings.is_empty(),
            "attested render has diagnostics"
        );
        ensure!(
            !output.svg.is_empty() && output.svg.contains("<svg"),
            "attested render produced no SVG"
        );
        self.verify_bindings()?;
        let mut artifacts = input_snapshots(&self.artifacts, input, &self.input_before.sha256)?;
        if let Some(capture) = &self.classic {
            artifacts["classicCapture"] = capture.verify()?;
        }
        write_new(&self.output, output.svg.as_bytes())?;
        let written_output = FileRecord::read(&self.output)?;
        ensure!(
            written_output.sha256 == sha256(output.svg.as_bytes()),
            "written output differs from rendered SVG"
        );
        write_new(&self.artifacts.join("output.svg"), output.svg.as_bytes())?;
        let output_snapshot = FileRecord::read(&self.artifacts.join("output.svg"))?;
        artifacts["output"] = output_snapshot.value();
        let receipt = self.receipt_value(input, output, &written_output, artifacts)?;
        /* WHY: 実終了コードは親がwaitで回収するため、CLI成功段階を終了証跡へ置き換えない。 */
        write_new(&self.receipt, &serde_json::to_vec_pretty(&receipt)?)
    }

    fn verify_bindings(&self) -> anyhow::Result<Value> {
        let input = FileRecord::read(&self.input_before.path)?;
        let runtime = FileRecord::read(&self.runtime_before.path)?;
        let executable = FileRecord::read(&self.executable_before.path)?;
        ensure!(
            input == self.input_before
                && runtime == self.runtime_before
                && executable == self.executable_before,
            "attestation input/runtime/executable changed during rendering"
        );
        Ok(json!({"input":input.value(),"runtime":runtime.value(),"executable":executable.value()}))
    }

    fn receipt_value(
        &self,
        input: &RenderInput,
        output: &RenderOutput,
        written: &FileRecord,
        artifacts: Value,
    ) -> anyhow::Result<Value> {
        let before = json!({"input":self.input_before.value(),"runtime":self.runtime_before.value(),"executable":self.executable_before.value()});
        let after = self.verify_bindings()?;
        let mut receipt = json!({
            "schema":"katana-native-render-attestation-v1",
            "status":"output-written",
            "accepted":false,
            "actualExit":null,
            "requiresExternalExitEvidence":true,
            "attemptToken":self.token,
            "pid":std::process::id(),
            "inputFormat":self.format.as_str(),
            "input":self.input_before.value(),
            "output":written.value(),
            "runtime":self.runtime_before.value(),
            "executable":self.executable_before.value(),
            "bindingsBefore":before,
            "bindingsAfter":after,
            "artifacts":artifacts
        });
        receipt::append_context(&mut receipt, input, output)?;
        Ok(receipt)
    }
}

#[cfg(test)]
#[path = "render_attestation_tests.rs"]
mod tests;
