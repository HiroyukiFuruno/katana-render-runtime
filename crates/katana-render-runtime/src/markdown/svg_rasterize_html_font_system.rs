use resvg::usvg;
use std::{collections::BTreeSet, path::PathBuf};

pub(super) fn load_requested_system_font_family(
    database: &mut usvg::fontdb::Database,
    family: &str,
) {
    for path in requested_system_font_paths(family) {
        database.load_font_source(usvg::fontdb::Source::File(path));
    }
}

fn requested_system_font_paths(family: &str) -> BTreeSet<PathBuf> {
    fontconfig_font_paths(family)
        .into_iter()
        .filter(|path| path.is_file())
        .collect()
}

fn fontconfig_font_paths(family: &str) -> BTreeSet<PathBuf> {
    parse_fontconfig_output(
        crate::system::ProcessService::create_command("fc-list")
            .args([
                "--format",
                "%{family}\\t%{file}\\n",
                &format!(":family={family}"),
            ])
            .output(),
        family,
    )
}

fn parse_fontconfig_output(
    output: std::io::Result<std::process::Output>,
    family: &str,
) -> BTreeSet<PathBuf> {
    let output = match output {
        Ok(output) if output.status.success() => output,
        _ => return BTreeSet::new(),
    };
    let output = match String::from_utf8(output.stdout) {
        Ok(output) => output,
        Err(_) => return BTreeSet::new(),
    };
    parse_fontconfig_font_paths(&output, family)
}

fn parse_fontconfig_font_paths(output: &str, requested_family: &str) -> BTreeSet<PathBuf> {
    output
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(families, _)| fontconfig_families_include(families, requested_family))
        .map(|(_, path)| PathBuf::from(path))
        .collect()
}

fn fontconfig_families_include(families: &str, requested_family: &str) -> bool {
    families
        .split(',')
        .any(|family| family.trim().eq_ignore_ascii_case(requested_family))
}

#[cfg(test)]
mod tests {
    use super::{
        fontconfig_families_include, load_requested_system_font_family,
        parse_fontconfig_font_paths, parse_fontconfig_output,
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
            "Courier New\t/tmp/courier-new.ttf\nCourier\t/tmp/courier.ttf\nCourier New,Courier\t/tmp/courier-new-bold.ttf\n",
            "courier new",
        );

        assert_eq!(
            paths,
            [
                PathBuf::from("/tmp/courier-new-bold.ttf"),
                PathBuf::from("/tmp/courier-new.ttf"),
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn fontconfig_output_rejects_process_failures_and_invalid_utf8() {
        let failed = parse_fontconfig_output(
            Err(std::io::Error::other("fontconfig unavailable")),
            "courier new",
        );
        assert!(failed.is_empty());

        let invalid_utf8 = parse_fontconfig_output(
            Ok(Output {
                status: ExitStatus::default(),
                stdout: vec![0xff],
                stderr: Vec::new(),
            }),
            "courier new",
        );
        assert!(invalid_utf8.is_empty());
    }

    #[test]
    fn unknown_requested_family_does_not_register_a_font() {
        let mut database = usvg::fontdb::Database::new();

        load_requested_system_font_family(&mut database, "krr-family-that-cannot-exist-7f0b");

        assert!(database.faces().next().is_none());
    }
}
