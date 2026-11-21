use super::{HTML_PNG_PROBE, RECURSIVE_PROBE, TestResult, assert_probe_rejected, fixture};
use crate::markdown::svg_rasterize::text::probe_test_support::TempProbeDirectory;

#[test]
fn public_font_and_svg_probes_reject_changed_fixture_bytes() -> TestResult<()> {
    let directory = TempProbeDirectory::create()?;
    for ((suffix, input_key, output_key), fixture_name, expected) in [
        (
            RECURSIVE_PROBE,
            "recursive-latin-crsv-normal.ttf",
            "unexpected SFNT input",
        ),
        (
            HTML_PNG_PROBE,
            "native-class-ja-probe.svg",
            "unexpected native SVG input",
        ),
    ] {
        let input = directory.file(fixture_name);
        let output = input.with_extension("observation");
        let mut bytes = std::fs::read(fixture(fixture_name)?)?;
        bytes.push(b' ');
        std::fs::write(&input, &bytes)?;
        assert_probe_rejected(
            suffix,
            &[(input_key, &input), (output_key, &output)],
            expected,
        )?;
        assert_eq!(std::fs::read(input)?, bytes);
        assert!(!output.exists());
    }
    Ok(())
}
