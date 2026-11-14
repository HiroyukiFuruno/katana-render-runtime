const MACH_HEADER_32_BYTES: usize = 28;
const MACH_HEADER_64_BYTES: usize = 32;
const SYMTAB_COMMAND_BYTES: usize = 24;
const NLIST_32_BYTES: usize = 12;
const NLIST_64_BYTES: usize = 16;
const STRING_TERMINATOR_COUNT: usize = 2;
const ZERO_NLIST_DESCRIPTION: [u8; 2] = [0; 2];
const ZERO_NLIST_VALUE_64: [u8; 8] = [0; 8];
const LC_SYMTAB: u32 = 2;
const INVALID_LOAD_COMMAND_SIZE: u32 = 0;
const MH_OBJECT: u32 = 1;
const CPU_TYPE_X86: u32 = 7;
const CPU_TYPE_X86_64: u32 = 0x0100_0007;
const CPU_SUBTYPE_X86_ALL: u32 = 3;
const N_OSO: u8 = 0x66;
const ONE_LOAD_COMMAND: u32 = 1;
const ONE_SYMBOL: u32 = 1;
const NO_FLAGS: u32 = 0;
const VALID_NAME_INDEX: u32 = 1;
const INVALID_NAME_INDEX: u32 = 1000;
const ZERO_SYMBOL_VALUE: u32 = 0;
const MACHO_32_MAGIC_LE: [u8; 4] = [0xce, 0xfa, 0xed, 0xfe];
const MACHO_64_MAGIC_LE: [u8; 4] = [0xcf, 0xfa, 0xed, 0xfe];
const FAT_MAGIC_BYTES: [u8; 4] = [0xca, 0xfe, 0xba, 0xbe];
const FAT_HEADER_BYTES: usize = 28;

#[derive(Clone, Copy)]
pub(crate) enum Corruption {
    None,
    LoadCommandSize,
    SymbolOffset,
    NameOffset,
    InvalidUtf8,
}

pub(crate) struct MachoNosoFixture;

impl MachoNosoFixture {
    pub(crate) fn thin_macho(is_64: bool, corruption: Corruption) -> Vec<u8> {
        let header_bytes = if is_64 {
            MACH_HEADER_64_BYTES
        } else {
            MACH_HEADER_32_BYTES
        };
        let nlist_bytes = if is_64 {
            NLIST_64_BYTES
        } else {
            NLIST_32_BYTES
        };
        let symbol_offset = header_bytes + SYMTAB_COMMAND_BYTES;
        let string_offset = symbol_offset + nlist_bytes;
        let name = match corruption {
            Corruption::InvalidUtf8 => vec![0xff],
            _ => b"/tmp/krr-object.o".to_vec(),
        };
        let mut output = mach_header(is_64);
        write_symtab_command(
            &mut output,
            symbol_offset,
            string_offset,
            name.len(),
            corruption,
        );
        write_nlist(&mut output, is_64, corruption);
        write_name_table(&mut output, &name);
        output
    }

    pub(crate) fn fat_macho() -> Vec<u8> {
        let mut bytes = FAT_MAGIC_BYTES.to_vec();
        bytes.extend_from_slice(&[0; FAT_HEADER_BYTES - FAT_MAGIC_BYTES.len()]);
        bytes
    }
}

fn mach_header(is_64: bool) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(if is_64 {
        &MACHO_64_MAGIC_LE
    } else {
        &MACHO_32_MAGIC_LE
    });
    push_u32(
        &mut output,
        if is_64 { CPU_TYPE_X86_64 } else { CPU_TYPE_X86 },
    );
    push_u32(&mut output, CPU_SUBTYPE_X86_ALL);
    push_u32(&mut output, MH_OBJECT);
    push_u32(&mut output, ONE_LOAD_COMMAND);
    push_u32(&mut output, SYMTAB_COMMAND_BYTES as u32);
    push_u32(&mut output, NO_FLAGS);
    if is_64 {
        push_u32(&mut output, NO_FLAGS);
    }
    output
}

fn write_symtab_command(
    output: &mut Vec<u8>,
    symbol_offset: usize,
    string_offset: usize,
    name_bytes: usize,
    corruption: Corruption,
) {
    push_u32(output, LC_SYMTAB);
    push_u32(
        output,
        if matches!(corruption, Corruption::LoadCommandSize) {
            INVALID_LOAD_COMMAND_SIZE
        } else {
            SYMTAB_COMMAND_BYTES as u32
        },
    );
    push_u32(
        output,
        if matches!(corruption, Corruption::SymbolOffset) {
            u32::MAX
        } else {
            symbol_offset as u32
        },
    );
    push_u32(output, ONE_SYMBOL);
    push_u32(output, string_offset as u32);
    push_u32(output, (name_bytes + STRING_TERMINATOR_COUNT) as u32);
}

fn write_nlist(output: &mut Vec<u8>, is_64: bool, corruption: Corruption) {
    push_u32(
        output,
        if matches!(corruption, Corruption::NameOffset) {
            INVALID_NAME_INDEX
        } else {
            VALID_NAME_INDEX
        },
    );
    output.extend_from_slice(&[N_OSO, 0]);
    output.extend_from_slice(&ZERO_NLIST_DESCRIPTION);
    if is_64 {
        output.extend_from_slice(&ZERO_NLIST_VALUE_64);
    } else {
        push_u32(output, ZERO_SYMBOL_VALUE);
    }
}

fn write_name_table(output: &mut Vec<u8>, name: &[u8]) {
    output.push(0);
    output.extend_from_slice(name);
    output.push(0);
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}
