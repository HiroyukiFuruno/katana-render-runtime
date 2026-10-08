use super::files::sha256;
use katana_render_runtime::{RenderInput, RenderOutput};
use serde_json::{Value, json};
use std::fs;

pub(super) fn append_context(
    receipt: &mut Value,
    input: &RenderInput,
    output: &RenderOutput,
) -> anyhow::Result<()> {
    receipt["actualRenderInput"] = serde_json::to_value(input)?;
    receipt["actualRenderSourceSha256"] = json!(sha256(input.source.as_bytes()));
    receipt["cwd"] = json!(fs::canonicalize(std::env::current_dir()?)?);
    receipt["tempDir"] = json!(fs::canonicalize(std::env::temp_dir())?);
    receipt["cliVersion"] = json!(env!("CARGO_PKG_VERSION"));
    receipt["runtimeMetadata"] = json!({"declaredRuntime":output.runtime,"profile":output.profile});
    receipt["diagnostics"] = serde_json::to_value(&output.diagnostics)?;
    receipt["scope"] = json!(
        "CLI RenderInput boundary; vendor prepared source/config and cache branch are not observed"
    );
    Ok(())
}
