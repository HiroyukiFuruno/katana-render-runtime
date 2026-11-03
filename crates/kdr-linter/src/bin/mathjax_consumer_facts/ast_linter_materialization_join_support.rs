use super::Fixture;
use std::path::Path;

pub(super) fn reject_install_path_join_mutations(
    fixture: &Fixture,
    original: &Path,
    renderer: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for (before, after) in [
        (".join(\"generated\")", ".join(\"forged-generated\")"),
        (
            ".join(\"katana-render-runtime\")",
            ".join(\"forged-runtime\")",
        ),
    ] {
        fixture.replace(renderer, before, after)?;
        fixture.reject("default-materialize:install-path-join")?;
        fixture.restore_from(renderer, original)?;
    }
    Ok(())
}
