use crate::macho_noso::MachoNoso;
use crate::macho_noso_test_support::{Corruption, MachoNosoFixture};

const SINGLE_NOSO_RECORD: usize = 1;
const FIRST_SYMBOL_INDEX: usize = 0;

#[test]
fn reads_noso_from_thin_mach_o_32_and_64() {
    for (is_64, expected_format) in [(false, "MachO32"), (true, "MachO64")] {
        let bytes = MachoNosoFixture::thin_macho(is_64, Corruption::None);
        let parsed = MachoNoso::records(&bytes);
        let summary =
            parsed.map(|(format, records)| (format, records.len(), records.first().cloned()));
        assert_eq!(
            summary,
            Ok((
                expected_format,
                SINGLE_NOSO_RECORD,
                Some(serde_json::json!({
                    "symbolIndex": FIRST_SYMBOL_INDEX,
                    "name": "/tmp/krr-object.o"
                }))
            ))
        );
    }
}

#[test]
fn rejects_fat_mach_o_and_bad_load_command_framing() {
    let fat = crate::macho_noso_test_support::MachoNosoFixture::fat_macho();
    assert!(MachoNoso::records(&fat).is_err());
    assert!(
        MachoNoso::records(&MachoNosoFixture::thin_macho(
            false,
            Corruption::LoadCommandSize
        ))
        .is_err()
    );
}

#[test]
fn rejects_bad_symbol_and_string_table_offsets() {
    assert!(
        MachoNoso::records(&MachoNosoFixture::thin_macho(
            true,
            Corruption::SymbolOffset
        ))
        .is_err()
    );
    assert!(
        MachoNoso::records(&MachoNosoFixture::thin_macho(true, Corruption::NameOffset)).is_err()
    );
}

#[test]
fn rejects_non_utf8_noso_names() {
    assert!(
        MachoNoso::records(&MachoNosoFixture::thin_macho(
            false,
            Corruption::InvalidUtf8
        ))
        .is_err()
    );
}
