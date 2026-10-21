use super::super::test_support::Fixture;
use std::path::Path;

const PLANTUML_CACHED_CLOSURE: &str = concat!(
    "|path| {\n",
    "                    PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)\n",
    "                }",
);
const PLANTUML_ARM: &str = concat!(
    "            DiagramKind::PlantUml => Ok(plantuml_cache_dir\n",
    "                .map_or_else(PlantUmlRendererOps::default_jar_path, |path| {\n",
    "                    PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)\n",
    "                })),\n",
);

const PLANTUML_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        PLANTUML_ARM,
        "            DiagramKind::PlantUml => plantuml_cache_dir,\n",
        "runtime-path-resolver:plantuml-arm",
    ),
    (
        "DiagramKind::PlantUml => Ok(plantuml_cache_dir",
        "DiagramKind::PlantUml => Err(plantuml_cache_dir",
        "runtime-path-resolver:plantuml-arm",
    ),
    (
        ".map_or_else(PlantUmlRendererOps::default_jar_path",
        ".map_or_else_fake(PlantUmlRendererOps::default_jar_path",
        "runtime-path-resolver:plantuml-map-or-else",
    ),
    (
        PLANTUML_CACHED_CLOSURE,
        "plantuml_mapper",
        "runtime-path-resolver:plantuml-arm",
    ),
    (
        "PlantUmlRendererOps::default_jar_path",
        "FakePlantUmlRendererOps::default_jar_path",
        "runtime-path-resolver:plantuml-default",
    ),
    (
        ".map_or_else(PlantUmlRendererOps::default_jar_path",
        ".map_or(PlantUmlRendererOps::default_jar_path",
        "runtime-path-resolver:plantuml-map-or-else",
    ),
    (
        "PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)",
        "PlantUmlRendererOps::default_jar_path_for_cache_dir(path)",
        "runtime-path-resolver:plantuml-cache",
    ),
    (
        "PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)",
        "PlantUmlRendererOps::default_jar_path_for_cache_dir(&path.join(\"nested\"))",
        "runtime-path-resolver:plantuml-cache",
    ),
];

pub(super) fn reject_mutations(
    fixture: &Fixture,
    resolver: &str,
    original: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    for (before, after, expected) in PLANTUML_MUTATIONS {
        fixture.replace(resolver, before, after)?;
        fixture.reject(expected)?;
        fixture.restore_from(resolver, original)?;
    }
    Ok(())
}
