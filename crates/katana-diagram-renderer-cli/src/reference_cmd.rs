use crate::system::ProcessService;
use anyhow::anyhow;
use katana_diagram_renderer::DiagramKind;
use std::ffi::OsString;
use std::path::PathBuf;

pub(crate) struct ReferenceCommand;

impl ReferenceCommand {
    pub(crate) fn update(kind: DiagramKind, fixtures: PathBuf) -> anyhow::Result<()> {
        JustRecipe::new(Self::recipe(kind, "reference"))
            .arg(fixtures)
            .run()
    }

    pub(crate) fn compare(
        kind: DiagramKind,
        fixtures: PathBuf,
        min_score: f32,
    ) -> anyhow::Result<()> {
        JustRecipe::new(Self::recipe(kind, "compare"))
            .arg(fixtures)
            .arg(min_score.to_string())
            .run()
    }

    pub(crate) fn bench(kind: DiagramKind, fixtures: PathBuf) -> anyhow::Result<()> {
        JustRecipe::new(Self::recipe(kind, "bench"))
            .arg(fixtures)
            .run()
    }

    fn recipe(kind: DiagramKind, action: &str) -> String {
        format!("{}-{action}", Self::name(kind))
    }

    fn name(kind: DiagramKind) -> &'static str {
        match kind {
            DiagramKind::Mermaid => "mermaid",
            DiagramKind::Drawio => "drawio",
        }
    }
}

struct JustRecipe {
    recipe: String,
    args: Vec<OsString>,
}

impl JustRecipe {
    fn new(recipe: String) -> Self {
        Self {
            recipe,
            args: Vec::new(),
        }
    }

    fn arg(mut self, value: impl Into<OsString>) -> Self {
        self.args.push(value.into());
        self
    }

    fn run(self) -> anyhow::Result<()> {
        let status = ProcessService::create_command("just")
            .arg(&self.recipe)
            .args(&self.args)
            .status()?;
        if status.success() {
            return Ok(());
        }
        Err(anyhow!("just recipe failed: {}", self.recipe))
    }
}

#[cfg(test)]
mod tests {
    use super::ReferenceCommand;
    use katana_diagram_renderer::DiagramKind;

    #[test]
    fn recipe_name_uses_diagram_prefix() {
        assert_eq!(
            ReferenceCommand::recipe(DiagramKind::Mermaid, "compare"),
            "mermaid-compare"
        );
        assert_eq!(
            ReferenceCommand::recipe(DiagramKind::Drawio, "reference"),
            "drawio-reference"
        );
    }
}
