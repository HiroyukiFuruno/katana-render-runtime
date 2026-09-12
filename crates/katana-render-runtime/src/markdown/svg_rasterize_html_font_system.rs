use resvg::usvg;
use std::{collections::BTreeSet, path::PathBuf};

/* WHY: 任意のHTMLが大量のfamilyを指定しても、同期したfontconfig探索は描画ごとに1回・有限件にする。 */
const MAX_SYSTEM_FONT_FAMILIES_PER_REQUEST: usize = 16;

pub(super) fn load_requested_system_font_families<'a>(
    database: &mut usvg::fontdb::Database,
    families: impl IntoIterator<Item = &'a str>,
) {
    let families = requested_system_font_families(families);
    if families.is_empty() {
        return;
    }
    load_system_font_paths(database, requested_system_font_paths(&families));
}

fn load_system_font_paths(
    database: &mut usvg::fontdb::Database,
    paths: impl IntoIterator<Item = PathBuf>,
) {
    for path in paths {
        database.load_font_source(usvg::fontdb::Source::File(path));
    }
}

fn requested_system_font_families<'a>(
    families: impl IntoIterator<Item = &'a str>,
) -> BTreeSet<&'a str> {
    let mut requested = BTreeSet::new();
    for family in families {
        if requested.len() == MAX_SYSTEM_FONT_FAMILIES_PER_REQUEST {
            break;
        }
        if !family.is_empty() {
            requested.insert(family);
        }
    }
    requested
}

fn requested_system_font_paths(families: &BTreeSet<&str>) -> BTreeSet<PathBuf> {
    existing_system_font_paths(fontconfig_font_paths(families))
}

fn existing_system_font_paths(paths: impl IntoIterator<Item = PathBuf>) -> BTreeSet<PathBuf> {
    paths.into_iter().filter(|path| path.is_file()).collect()
}

fn fontconfig_font_paths(families: &BTreeSet<&str>) -> BTreeSet<PathBuf> {
    parse_fontconfig_output(
        crate::system::ProcessService::create_command("fc-list")
            .args(["--format", "%{family}\\t%{file}\\n"])
            .output(),
        families,
    )
}

fn parse_fontconfig_output(
    output: std::io::Result<std::process::Output>,
    families: &BTreeSet<&str>,
) -> BTreeSet<PathBuf> {
    let output = match output {
        Ok(output) if output.status.success() => output,
        _ => return BTreeSet::new(),
    };
    let output = match String::from_utf8(output.stdout) {
        Ok(output) => output,
        Err(_) => return BTreeSet::new(),
    };
    parse_fontconfig_font_paths(&output, families)
}

fn parse_fontconfig_font_paths(
    output: &str,
    requested_families: &BTreeSet<&str>,
) -> BTreeSet<PathBuf> {
    output
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(families, _)| fontconfig_families_include_any(families, requested_families))
        .map(|(_, path)| PathBuf::from(path))
        .collect()
}

fn fontconfig_families_include(families: &str, requested_family: &str) -> bool {
    families
        .split(',')
        .any(|family| family.trim().eq_ignore_ascii_case(requested_family))
}

fn fontconfig_families_include_any(families: &str, requested_families: &BTreeSet<&str>) -> bool {
    requested_families
        .iter()
        .any(|requested_family| fontconfig_families_include(families, requested_family))
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_SYSTEM_FONT_FAMILIES_PER_REQUEST, existing_system_font_paths,
        fontconfig_families_include, load_requested_system_font_families, load_system_font_paths,
        parse_fontconfig_font_paths, parse_fontconfig_output, requested_system_font_families,
    };
    use resvg::usvg;
    use std::{
        path::PathBuf,
        process::{ExitStatus, Output},
    };

    #[test]
    fn fontconfig_family_match_requires_the_exact_requested_family() {
        assert!(fontconfig_families_include(
            "Courier New,Courier",
            "courier new"
        ));
        assert!(!fontconfig_families_include("Courier", "courier new"));
    }

    #[test]
    fn fontconfig_parser_selects_only_exact_family_paths() {
        let paths = parse_fontconfig_font_paths(
            "Courier New\t/tmp/courier-new.ttf\nCourier\t/tmp/courier.ttf\nArial\t/tmp/arial.ttf\nCourier New,Courier\t/tmp/courier-new-bold.ttf\n",
            &["courier new", "arial"].into_iter().collect(),
        );

        assert_eq!(
            paths,
            [
                PathBuf::from("/tmp/courier-new-bold.ttf"),
                PathBuf::from("/tmp/courier-new.ttf"),
                PathBuf::from("/tmp/arial.ttf"),
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn fontconfig_output_rejects_process_failures_and_invalid_utf8() {
        let failed = parse_fontconfig_output(
            Err(std::io::Error::other("fontconfig unavailable")),
            &["courier new"].into_iter().collect(),
        );
        assert!(failed.is_empty());

        let invalid_utf8 = parse_fontconfig_output(
            Ok(Output {
                status: ExitStatus::default(),
                stdout: vec![0xff],
                stderr: Vec::new(),
            }),
            &["courier new"].into_iter().collect(),
        );
        assert!(invalid_utf8.is_empty());
    }

    #[test]
    fn fontconfig_output_selects_an_exact_family_from_a_successful_process() {
        let paths = parse_fontconfig_output(
            Ok(Output {
                status: ExitStatus::default(),
                stdout: b"Courier New\t/tmp/courier-new.ttf\nArial\t/tmp/arial.ttf\n".to_vec(),
                stderr: Vec::new(),
            }),
            &["courier new", "arial"].into_iter().collect(),
        );

        assert_eq!(
            paths,
            [
                PathBuf::from("/tmp/arial.ttf"),
                PathBuf::from("/tmp/courier-new.ttf"),
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn system_font_path_loader_registers_a_known_bundled_font() {
        let font =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf");
        let mut database = usvg::fontdb::Database::new();

        load_system_font_paths(&mut database, [font]);

        assert!(database.faces().next().is_some());
    }

    #[test]
    fn existing_system_font_paths_discards_missing_paths() {
        let existing =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf");
        let paths = existing_system_font_paths([
            existing.clone(),
            PathBuf::from("/krr-font-path-that-does-not-exist.ttf"),
        ]);

        assert_eq!(paths, [existing].into_iter().collect());
    }

    #[test]
    fn system_font_resolution_bounds_one_request() {
        let mut families = (0..=MAX_SYSTEM_FONT_FAMILIES_PER_REQUEST)
            .map(|index| format!("krr-font-{index}"))
            .collect::<Vec<_>>();
        families.push("krr-font-0".to_string());

        let requested = requested_system_font_families(families.iter().map(String::as_str));

        assert_eq!(requested.len(), MAX_SYSTEM_FONT_FAMILIES_PER_REQUEST);
        assert!(requested.contains("krr-font-0"));
        assert!(
            !requested
                .contains(&format!("krr-font-{MAX_SYSTEM_FONT_FAMILIES_PER_REQUEST}").as_str())
        );
    }

    #[test]
    fn unknown_requested_families_do_not_register_a_font() {
        let mut database = usvg::fontdb::Database::new();

        load_requested_system_font_families(&mut database, ["krr-family-that-cannot-exist-7f0b"]);

        assert!(database.faces().next().is_none());
    }
}
