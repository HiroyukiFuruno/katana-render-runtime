use super::{Corruption, MachOReader, MachoNosoFixture, header_bytes};

const LOAD_COMMAND_ERROR_PREFIXES: &[&str] = &["macho-load-command:", "macho-load-commands:"];
const CORRUPTED_FIXTURE_CASES: &[(bool, Corruption, &[&str])] = &[
    (
        false,
        Corruption::LoadCommandSize,
        LOAD_COMMAND_ERROR_PREFIXES,
    ),
    (
        true,
        Corruption::LoadCommandSize,
        LOAD_COMMAND_ERROR_PREFIXES,
    ),
    (false, Corruption::SymbolOffset, &["macho-symbol-table:"]),
    (true, Corruption::SymbolOffset, &["macho-symbol-table:"]),
    (false, Corruption::NameOffset, &["macho-noso-string:"]),
    (true, Corruption::NameOffset, &["macho-noso-string:"]),
    (
        false,
        Corruption::InvalidUtf8,
        &["macho-noso-string:invalid-utf8"],
    ),
    (
        true,
        Corruption::InvalidUtf8,
        &["macho-noso-string:invalid-utf8"],
    ),
];
const INCOMPLETE_MAGIC_BYTES: usize = 3;
const HEADER_TRUNCATION_BYTES: usize = 1;
const SYMTAB_COMMAND_BYTES: usize = 24;
const THIN_ONLY_ERROR: &str = "macho-kind:thin-32-or-64-required";

#[test]
fn reads_big_endian_thin_macho_symbols() -> Result<(), String> {
    for (is_64, format) in [(false, "MachO32"), (true, "MachO64")] {
        let mut bytes = MachoNosoFixture::thin_macho(is_64, Corruption::None);
        let symbol_offset = header_bytes(is_64) + SYMTAB_COMMAND_BYTES;
        for word in bytes[..symbol_offset + 4].chunks_exact_mut(4) {
            word.reverse();
        }
        let (actual_format, records) = MachOReader::records(&bytes)?;
        assert_eq!(actual_format, format);
        assert_eq!(
            records,
            vec![serde_json::json!({"symbolIndex": 0, "name": "/tmp/krr-object.o"})]
        );
    }
    Ok(())
}

#[test]
fn rejects_invalid_magic() -> Result<(), String> {
    for is_64 in [false, true] {
        let mut bytes = MachoNosoFixture::thin_macho(is_64, Corruption::None);
        bytes[..4].fill(0);
        assert_error_prefix(&bytes, "macho-kind:")?;
    }
    Ok(())
}

#[test]
fn rejects_truncated_symbol_command_and_oversized_command_region() -> Result<(), String> {
    for is_64 in [false, true] {
        let valid = MachoNosoFixture::thin_macho(is_64, Corruption::None);
        assert!(MachOReader::records(&valid).is_ok());
        let mut short_command = valid.clone();
        let command_size = header_bytes(is_64) + 4;
        short_command[command_size..command_size + 4].copy_from_slice(&8_u32.to_le_bytes());
        assert_error_prefix(&short_command, "macho-symtab-command:")?;
        let mut oversized_commands = valid;
        oversized_commands[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_error_prefix(&oversized_commands, "macho-load-commands:")?;
    }
    Ok(())
}

#[test]
fn rejects_corrupted_thin_macho32_and_macho64_inputs() -> Result<(), String> {
    for (is_64, corruption, expected_prefixes) in CORRUPTED_FIXTURE_CASES {
        let bytes = MachoNosoFixture::thin_macho(*is_64, *corruption);
        assert_error_prefixes(&bytes, expected_prefixes)?;
    }
    Ok(())
}

#[test]
fn rejects_fat_macho_and_truncated_header_boundaries() -> Result<(), String> {
    assert_eq!(
        MachOReader::records(&MachoNosoFixture::fat_macho())
            .err()
            .as_deref(),
        Some(THIN_ONLY_ERROR)
    );
    assert_error_prefix(
        &MachoNosoFixture::thin_macho(false, Corruption::None)[..INCOMPLETE_MAGIC_BYTES],
        "macho-kind:",
    )?;
    assert_truncated_headers_rejected()?;
    Ok(())
}

fn assert_truncated_headers_rejected() -> Result<(), String> {
    for is_64 in [false, true] {
        let mut bytes = MachoNosoFixture::thin_macho(is_64, Corruption::None);
        bytes.truncate(header_bytes(is_64) - HEADER_TRUNCATION_BYTES);
        assert_error_prefix(&bytes, "macho-header:")?;
    }
    Ok(())
}

fn assert_error_prefix(bytes: &[u8], expected_prefix: &str) -> Result<(), String> {
    assert_error_prefixes(bytes, &[expected_prefix])
}

fn assert_error_prefixes(bytes: &[u8], expected_prefixes: &[&str]) -> Result<(), String> {
    let error = MachOReader::records(bytes)
        .err()
        .ok_or("malformed Mach-O input was accepted")?;
    if !expected_prefixes
        .iter()
        .any(|prefix| error.starts_with(prefix))
    {
        return Err(format!("unexpected Mach-O rejection: {error}"));
    }
    Ok(())
}
