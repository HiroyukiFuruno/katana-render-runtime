use super::{MermaidJsRuntimeOps, RuntimeAsset};
use crate::markdown::color_preset::DiagramColorPreset;

const GIT_GRAPH_SOURCE: &str = "gitGraph
    commit id: \"base\"
    branch feature
    checkout feature
    commit id: \"rust-js\"
    checkout main
    merge feature";
const OFFICIAL_VIEW_BOX: [f64; 4] = [-114.078_125, -21.5, 272.078_125, 161.971_252_441_406_25];
const OFFICIAL_SVG_SHA256: &str =
    "1bf9d5313fe8f351dfebea7b6c77519f968dd1c7bf157954f3dbf4514a960099";

#[test]
fn gitgraph_rotated_commit_label_matches_official_view_box() -> Result<(), String> {
    let asset = RuntimeAsset::mermaid();
    let rendered = asset
        .materialize_at(asset.materialized_path())
        .and_then(|bundle| {
            MermaidJsRuntimeOps::render(GIT_GRAPH_SOURCE, &bundle, DiagramColorPreset::dark())
        });
    let svg = rendered.as_ref().map_err(|error| error.to_owned())?;
    super::super::reference_score_tests::assert_view_box_matches(
        svg,
        OFFICIAL_VIEW_BOX,
        OFFICIAL_SVG_SHA256,
    )
}
