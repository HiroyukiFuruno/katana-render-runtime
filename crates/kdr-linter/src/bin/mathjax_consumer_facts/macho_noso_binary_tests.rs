use super::MachOReader;
use crate::macho_noso_test_support::{Corruption, MachoNosoFixture};
use std::mem::size_of;

const MACH_HEADER_32_BYTES: usize = 28;
const MACH_HEADER_64_BYTES: usize = 32;
const MACH_HEADER_NCMDS_OFFSET: usize = 16;
const MACH_HEADER_SIZEOF_CMDS_OFFSET: usize = 20;
const SYMTAB_COMMAND_BYTES: usize = 24;
const SYMTAB_COMMAND_SYMOFF_OFFSET: usize = 8;
const SYMTAB_COMMAND_STROFF_OFFSET: usize = 16;
const NLIST_32_BYTES: usize = 12;
const NLIST_64_BYTES: usize = 16;
const NLIST_NAME_INDEX_BYTES: usize = size_of::<u32>();
const NLIST_N_TYPE_OFFSET: usize = NLIST_NAME_INDEX_BYTES;
const SYMTAB_COMMAND_BASE_OFFSET: usize = 0;
const DUPLICATE_SYMTAB_COUNT: u32 = 2;
const DUPLICATE_COMMAND_BYTES: u32 =
    (SYMTAB_COMMAND_BYTES * DUPLICATE_SYMTAB_COUNT as usize) as u32;

#[path = "macho_noso_binary_boundary_tests.rs"]
mod boundary_tests;

#[test]
fn accepts_non_noso_symbols_and_reports_missing_or_duplicate_symtabs() {
    for (is_64, expected_format) in [(false, "MachO32"), (true, "MachO64")] {
        let header_size = header_bytes(is_64);
        let symbol_offset = header_size + SYMTAB_COMMAND_BYTES;
        let mut non_noso = MachoNosoFixture::thin_macho(is_64, Corruption::None);
        non_noso[symbol_offset + NLIST_N_TYPE_OFFSET] = 0;
        assert_eq!(
            MachOReader::records(&non_noso),
            Ok((expected_format, Vec::new()))
        );

        let mut missing = MachoNosoFixture::thin_macho(is_64, Corruption::None);
        missing[header_size] = 0;
        assert_eq!(
            MachOReader::records(&missing),
            Ok((expected_format, Vec::new()))
        );

        let duplicate = duplicate_symtab_fixture(is_64);
        assert_eq!(
            MachOReader::records(&duplicate).err().as_deref(),
            Some("macho-symtab:duplicate")
        );
    }
}

#[test]
fn rejects_empty_noso_names() {
    for is_64 in [false, true] {
        let mut bytes = MachoNosoFixture::thin_macho(is_64, Corruption::None);
        let symbol_offset = header_bytes(is_64) + SYMTAB_COMMAND_BYTES;
        bytes[symbol_offset..symbol_offset + NLIST_NAME_INDEX_BYTES].fill(0);
        assert_eq!(
            MachOReader::records(&bytes).err().as_deref(),
            Some("macho-noso-string:empty-or-embedded-nul")
        );
    }
}

fn duplicate_symtab_fixture(is_64: bool) -> Vec<u8> {
    let original = MachoNosoFixture::thin_macho(is_64, Corruption::None);
    let header_size = header_bytes(is_64);
    let nlist_size = if is_64 {
        NLIST_64_BYTES
    } else {
        NLIST_32_BYTES
    };
    let command_start = header_size;
    let payload_start = command_start + SYMTAB_COMMAND_BYTES;
    let first_symbol_offset =
        (header_size + SYMTAB_COMMAND_BYTES * DUPLICATE_SYMTAB_COUNT as usize) as u32;
    let first_string_offset = first_symbol_offset + nlist_size as u32;
    let mut command = original[command_start..payload_start].to_vec();
    write_symtab_offsets(
        &mut command,
        SYMTAB_COMMAND_BASE_OFFSET,
        first_symbol_offset,
        first_string_offset,
    );
    let mut bytes = original[..header_size].to_vec();
    bytes.extend_from_slice(&command);
    bytes.extend_from_slice(&command);
    bytes.extend_from_slice(&original[payload_start..]);
    bytes[MACH_HEADER_NCMDS_OFFSET..MACH_HEADER_NCMDS_OFFSET + size_of::<u32>()]
        .copy_from_slice(&DUPLICATE_SYMTAB_COUNT.to_le_bytes());
    bytes[MACH_HEADER_SIZEOF_CMDS_OFFSET..MACH_HEADER_SIZEOF_CMDS_OFFSET + size_of::<u32>()]
        .copy_from_slice(&DUPLICATE_COMMAND_BYTES.to_le_bytes());
    bytes
}

fn header_bytes(is_64: bool) -> usize {
    if is_64 {
        MACH_HEADER_64_BYTES
    } else {
        MACH_HEADER_32_BYTES
    }
}

fn write_symtab_offsets(
    bytes: &mut [u8],
    command_offset: usize,
    symbol_offset: u32,
    string_offset: u32,
) {
    bytes[command_offset + SYMTAB_COMMAND_SYMOFF_OFFSET
        ..command_offset + SYMTAB_COMMAND_SYMOFF_OFFSET + size_of::<u32>()]
        .copy_from_slice(&symbol_offset.to_le_bytes());
    bytes[command_offset + SYMTAB_COMMAND_STROFF_OFFSET
        ..command_offset + SYMTAB_COMMAND_STROFF_OFFSET + size_of::<u32>()]
        .copy_from_slice(&string_offset.to_le_bytes());
}
