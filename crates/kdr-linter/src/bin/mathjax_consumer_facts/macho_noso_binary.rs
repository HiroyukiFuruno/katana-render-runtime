use object::FileKind;
use object::endian::Endianness;
use object::macho;
use object::read::macho::{MachHeader, Nlist};
use serde_json::{Value, json};

const ZERO_FILE_OFFSET: u64 = 0;

type NosoSymbolTable<'data, Mach> = object::read::macho::SymbolTable<'data, Mach, &'data [u8]>;

pub(crate) struct MachOReader;

impl MachOReader {
    pub(crate) fn records(bytes: &[u8]) -> Result<(&'static str, Vec<Value>), String> {
        let kind = FileKind::parse(bytes).map_err(|error| format!("macho-kind:{error}"))?;
        match kind {
            FileKind::MachO32 => {
                Self::extract_format::<macho::MachHeader32<Endianness>>("MachO32", bytes)
            }
            FileKind::MachO64 => {
                Self::extract_format::<macho::MachHeader64<Endianness>>("MachO64", bytes)
            }
            _ => Err("macho-kind:thin-32-or-64-required".into()),
        }
    }

    fn extract_format<Mach>(
        format: &'static str,
        bytes: &[u8],
    ) -> Result<(&'static str, Vec<Value>), String>
    where
        Mach: MachHeader<Endian = Endianness>,
    {
        Ok((format, Self::extract_noso::<Mach>(bytes)?))
    }

    fn extract_noso<Mach>(bytes: &[u8]) -> Result<Vec<Value>, String>
    where
        Mach: MachHeader<Endian = Endianness>,
    {
        let header = Mach::parse(bytes, ZERO_FILE_OFFSET)
            .map_err(|error| format!("macho-header:{error}"))?;
        let endian = header
            .endian()
            .map_err(|error| format!("macho-endian:{error}"))?;
        let Some(table) = Self::read_symbol_table(header, endian, bytes)? else {
            return Ok(Vec::new());
        };
        let mut records = Vec::new();
        for (index, symbol) in table.iter().enumerate() {
            if symbol.stab() != Some(macho::N_OSO) {
                continue;
            }
            let name = symbol
                .name(endian, table.strings())
                .map_err(|error| format!("macho-noso-string:{error}"))?;
            records.push(Self::noso_record(index, name)?);
        }
        Ok(records)
    }

    fn read_symbol_table<'data, Mach>(
        header: &'data Mach,
        endian: Endianness,
        bytes: &'data [u8],
    ) -> Result<Option<NosoSymbolTable<'data, Mach>>, String>
    where
        Mach: MachHeader<Endian = Endianness>,
    {
        let mut commands = header
            .load_commands(endian, bytes, ZERO_FILE_OFFSET)
            .map_err(|error| format!("macho-load-commands:{error}"))?;
        let mut symbol_table = None;
        while let Some(command) = commands
            .next()
            .map_err(|error| format!("macho-load-command:{error}"))?
        {
            symbol_table = Self::read_symtab_command(command, endian, bytes, symbol_table)?;
        }
        Ok(symbol_table)
    }

    fn read_symtab_command<'data, Mach>(
        command: object::read::macho::LoadCommandData<'data, Endianness>,
        endian: Endianness,
        bytes: &'data [u8],
        current: Option<NosoSymbolTable<'data, Mach>>,
    ) -> Result<Option<NosoSymbolTable<'data, Mach>>, String>
    where
        Mach: MachHeader<Endian = Endianness>,
    {
        let Some(symtab) = command
            .symtab()
            .map_err(|error| format!("macho-symtab-command:{error}"))?
        else {
            return Ok(current);
        };
        if current.is_some() {
            return Err("macho-symtab:duplicate".into());
        }
        symtab
            .symbols::<Mach, &[u8]>(endian, bytes)
            .map(Some)
            .map_err(|error| format!("macho-symbol-table:{error}"))
    }

    fn noso_record(index: usize, raw_name: &[u8]) -> Result<Value, String> {
        let name = std::str::from_utf8(raw_name).map_err(|_| "macho-noso-string:invalid-utf8")?;
        if name.is_empty() || name.contains('\0') {
            return Err("macho-noso-string:empty-or-embedded-nul".into());
        }
        Ok(json!({"symbolIndex": index, "name": name}))
    }
}

#[cfg(test)]
#[path = "macho_noso_binary_tests.rs"]
mod tests;
