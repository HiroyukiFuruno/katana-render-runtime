use super::{HtmlRenderInput, HtmlRenderer};
use base64::Engine as _;

type TestResult<T = ()> = Result<T, String>;

const LARGE_IDAT_BYTES: usize = 256 * 1024;
const LARGE_PNG_WIDTH: u32 = 16_383;
const LARGE_PNG_HEIGHT: u32 = 16;
const ADLER32_MODULUS: usize = 65_521;
const ADLER32_HIGH_SHIFT: u32 = 16;
const PNG_CHUNK_TYPE_BYTES: usize = 4;
const PNG_IHDR_BYTES: usize = 13;
const PNG_WIDTH_OFFSET: usize = 0;
const PNG_WIDTH_END: usize = 4;
const PNG_IHDR_HEIGHT_OFFSET: usize = 4;
const PNG_IHDR_HEIGHT_END: usize = 8;
const PNG_BIT_DEPTH_OFFSET: usize = 8;
const PNG_GRAYSCALE_BIT_DEPTH: u8 = 8;
const PNG_ONE_BIT_DEPTH: u8 = 1;
const PNG_CRC32_POLYNOMIAL: u32 = 0xedb8_8320;
const BITS_PER_BYTE: usize = 8;
const PNG_SINGLE_PIXEL_DIMENSION: u32 = 1;
const HIGHLY_COMPRESSIBLE_PNG_WIDTH: u32 = 16_384;
const HIGHLY_COMPRESSIBLE_PNG_HEIGHT: u32 = 16_384;
const DEFLATE_MAX_MATCH_LENGTH: usize = 258;
const DEFLATE_FINAL_BLOCK: usize = 1;
const DEFLATE_FIXED_HUFFMAN_BLOCK: usize = 1;
const DEFLATE_FINAL_BIT_COUNT: usize = 1;
const DEFLATE_BLOCK_TYPE_BIT_COUNT: usize = 2;
const DEFLATE_FIRST_ZERO_SYMBOL: usize = 0;
const DEFLATE_END_OF_BLOCK_SYMBOL: usize = 256;
const DEFLATE_LENGTH_258_SYMBOL: usize = 285;
const DEFLATE_LENGTH_113_SYMBOL: usize = 279;
const DEFLATE_LENGTH_113_BASE: usize = 99;
const DEFLATE_LENGTH_113_MAX: usize = 114;
const DEFLATE_LENGTH_113_EXTRA_BITS: usize = 4;
const DEFLATE_DISTANCE_ONE_SYMBOL: usize = 0;
const DEFLATE_DISTANCE_SYMBOL_BITS: usize = 5;
const FIXED_LITERAL_MAX_SYMBOL: usize = 143;
const FIXED_LITERAL_CODE_BASE: usize = 0x30;
const FIXED_LITERAL_CODE_BITS: usize = 8;
const FIXED_LENGTH_MIN_SYMBOL: usize = 144;
const FIXED_LENGTH_MAX_SYMBOL: usize = 255;
const FIXED_LENGTH_CODE_BASE: usize = 0x190;
const FIXED_LENGTH_CODE_BITS: usize = 9;
const FIXED_CODE_MIN_SYMBOL: usize = 256;
const FIXED_CODE_MAX_SYMBOL: usize = 279;
const FIXED_CODE_CODE_BITS: usize = 7;
const FIXED_DISTANCE_MIN_SYMBOL: usize = 280;
const FIXED_DISTANCE_MAX_SYMBOL: usize = 287;
const FIXED_DISTANCE_CODE_BASE: usize = 0xc0;
const FIXED_DISTANCE_CODE_BITS: usize = 8;
const ZLIB_HEADER: [u8; 2] = [0x78, 0x01];
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const IHDR_CHUNK_TYPE: &[u8; PNG_CHUNK_TYPE_BYTES] = b"IHDR";
const IDAT_CHUNK_TYPE: &[u8; PNG_CHUNK_TYPE_BYTES] = b"IDAT";
const IEND_CHUNK_TYPE: &[u8; PNG_CHUNK_TYPE_BYTES] = b"IEND";

#[test]
fn dispatches_error_for_png_with_empty_inflated_scanlines() -> TestResult {
    let payload = base64::engine::general_purpose::STANDARD.encode(png_with_raw_scanlines(
        &[],
        PNG_SINGLE_PIXEL_DIMENSION,
        PNG_SINGLE_PIXEL_DIMENSION,
    ));
    let output = render(&format!(
        r#"<p id=status></p><img src="data:image/png;base64,{payload}" onload="document.getElementById('status').textContent = 'load'" onerror="document.getElementById('status').textContent = 'error'">"#
    ))?;

    assert!(output.contains(">error</p>"), "{output}");
    assert!(!output.contains(">load</p>"), "{output}");
    Ok(())
}

#[test]
fn dispatches_error_for_highly_compressible_png_over_the_shared_decoder_limit() -> TestResult {
    let payload =
        base64::engine::general_purpose::STANDARD.encode(highly_compressible_16k_1bit_png()?);
    let output = render(&format!(
        r#"<p id=status></p><img src="data:image/png;base64,{payload}" onload="document.getElementById('status').textContent = 'load'" onerror="document.getElementById('status').textContent = 'error'">"#
    ))?;

    assert!(output.contains(">error</p>"), "{output}");
    assert!(!output.contains(">load</p>"), "{output}");
    Ok(())
}

#[test]
fn dispatches_load_for_large_single_idat_png_data_urls() -> TestResult {
    let payload = base64::engine::general_purpose::STANDARD.encode(large_single_idat_png());
    let output = render(&format!(
        r#"<p id=status></p><img src="data:image/png;base64,{payload}" onload="document.getElementById('status').textContent = 'load'" onerror="document.getElementById('status').textContent = 'error'">"#
    ))?;

    assert!(output.contains(">load</p>"), "{output}");
    Ok(())
}

fn large_single_idat_png() -> Vec<u8> {
    png_with_raw_scanlines(
        &vec![0; LARGE_IDAT_BYTES],
        LARGE_PNG_WIDTH,
        LARGE_PNG_HEIGHT,
    )
}

fn highly_compressible_16k_1bit_png() -> TestResult<Vec<u8>> {
    let decoded_len = (HIGHLY_COMPRESSIBLE_PNG_WIDTH as usize / BITS_PER_BYTE + 1)
        * HIGHLY_COMPRESSIBLE_PNG_HEIGHT as usize;
    let mut png = PNG_SIGNATURE.to_vec();
    let mut header = [0; PNG_IHDR_BYTES];
    header[PNG_WIDTH_OFFSET..PNG_WIDTH_END]
        .copy_from_slice(&HIGHLY_COMPRESSIBLE_PNG_WIDTH.to_be_bytes());
    header[PNG_IHDR_HEIGHT_OFFSET..PNG_IHDR_HEIGHT_END]
        .copy_from_slice(&HIGHLY_COMPRESSIBLE_PNG_HEIGHT.to_be_bytes());
    header[PNG_BIT_DEPTH_OFFSET] = PNG_ONE_BIT_DEPTH;
    append_png_chunk(&mut png, IHDR_CHUNK_TYPE, &header);
    append_png_chunk(
        &mut png,
        IDAT_CHUNK_TYPE,
        &compressible_zlib_payload(decoded_len)?,
    );
    append_png_chunk(&mut png, IEND_CHUNK_TYPE, &[]);
    Ok(png)
}

fn compressible_zlib_payload(decoded_len: usize) -> TestResult<Vec<u8>> {
    let mut deflate = FixedDeflateWriter::default();
    deflate.write_bits(DEFLATE_FINAL_BLOCK, DEFLATE_FINAL_BIT_COUNT);
    deflate.write_bits(DEFLATE_FIXED_HUFFMAN_BLOCK, DEFLATE_BLOCK_TYPE_BIT_COUNT);
    deflate.write_fixed_symbol(DEFLATE_FIRST_ZERO_SYMBOL)?;
    write_zero_match_run(&mut deflate, decoded_len - 1)?;
    deflate.write_fixed_symbol(DEFLATE_END_OF_BLOCK_SYMBOL)?;

    let mut zlib = ZLIB_HEADER.to_vec();
    zlib.extend(deflate.finish());
    let adler32 = (((decoded_len % ADLER32_MODULUS) as u32) << ADLER32_HIGH_SHIFT) | 1;
    zlib.extend_from_slice(&adler32.to_be_bytes());
    Ok(zlib)
}

fn write_zero_match_run(deflate: &mut FixedDeflateWriter, repeated: usize) -> TestResult {
    let full_matches = repeated / DEFLATE_MAX_MATCH_LENGTH;
    let remainder = repeated % DEFLATE_MAX_MATCH_LENGTH;
    for _ in 0..full_matches {
        deflate.write_fixed_symbol(DEFLATE_LENGTH_258_SYMBOL)?;
        deflate.write_bits(DEFLATE_DISTANCE_ONE_SYMBOL, DEFLATE_DISTANCE_SYMBOL_BITS);
    }
    if remainder > 0 {
        let (symbol, extra, extra_bits) = match remainder {
            DEFLATE_LENGTH_113_BASE..=DEFLATE_LENGTH_113_MAX => (
                DEFLATE_LENGTH_113_SYMBOL,
                remainder - DEFLATE_LENGTH_113_BASE,
                DEFLATE_LENGTH_113_EXTRA_BITS,
            ),
            _ => return Err("fixture remainder must fit one fixed length symbol".to_string()),
        };
        deflate.write_fixed_symbol(symbol)?;
        deflate.write_bits(extra, extra_bits);
        deflate.write_bits(DEFLATE_DISTANCE_ONE_SYMBOL, DEFLATE_DISTANCE_SYMBOL_BITS);
    }
    Ok(())
}

#[derive(Default)]
struct FixedDeflateWriter {
    bytes: Vec<u8>,
    bit_offset: usize,
}

impl FixedDeflateWriter {
    fn write_bits(&mut self, value: usize, count: usize) {
        for bit in 0..count {
            let byte_offset = self.bit_offset / BITS_PER_BYTE;
            if byte_offset == self.bytes.len() {
                self.bytes.push(0);
            }
            if value & (1 << bit) != 0 {
                self.bytes[byte_offset] |= 1 << (self.bit_offset % BITS_PER_BYTE);
            }
            self.bit_offset += 1;
        }
    }

    fn write_fixed_symbol(&mut self, symbol: usize) -> TestResult {
        let (code, bit_count) = match symbol {
            DEFLATE_FIRST_ZERO_SYMBOL..=FIXED_LITERAL_MAX_SYMBOL => {
                (FIXED_LITERAL_CODE_BASE + symbol, FIXED_LITERAL_CODE_BITS)
            }
            FIXED_LENGTH_MIN_SYMBOL..=FIXED_LENGTH_MAX_SYMBOL => (
                FIXED_LENGTH_CODE_BASE + symbol - FIXED_LENGTH_MIN_SYMBOL,
                FIXED_LENGTH_CODE_BITS,
            ),
            FIXED_CODE_MIN_SYMBOL..=FIXED_CODE_MAX_SYMBOL => {
                (symbol - FIXED_CODE_MIN_SYMBOL, FIXED_CODE_CODE_BITS)
            }
            FIXED_DISTANCE_MIN_SYMBOL..=FIXED_DISTANCE_MAX_SYMBOL => (
                FIXED_DISTANCE_CODE_BASE + symbol - FIXED_DISTANCE_MIN_SYMBOL,
                FIXED_DISTANCE_CODE_BITS,
            ),
            _ => return Err("symbol must be in the fixed Huffman alphabet".to_string()),
        };
        let reversed =
            (0..bit_count).fold(0, |reversed, bit| (reversed << 1) | ((code >> bit) & 1));
        self.write_bits(reversed, bit_count);
        Ok(())
    }

    fn finish(mut self) -> Vec<u8> {
        let required_bytes = self.bit_offset.div_ceil(BITS_PER_BYTE);
        self.bytes.resize(required_bytes, 0);
        self.bytes
    }
}

fn png_with_raw_scanlines(raw: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut zlib = ZLIB_HEADER.to_vec();
    let mut offset = 0;
    loop {
        let remaining = raw.len() - offset;
        let block_len = remaining.min(u16::MAX as usize);
        let is_last = block_len == remaining;
        zlib.push(u8::from(is_last));
        zlib.extend_from_slice(&(block_len as u16).to_le_bytes());
        zlib.extend_from_slice(&(!(block_len as u16)).to_le_bytes());
        zlib.extend_from_slice(&raw[offset..offset + block_len]);
        offset += block_len;
        if is_last {
            break;
        }
    }
    zlib.extend_from_slice(&adler32(raw).to_be_bytes());

    let mut png = PNG_SIGNATURE.to_vec();
    let mut header = [0; PNG_IHDR_BYTES];
    header[PNG_WIDTH_OFFSET..PNG_WIDTH_END].copy_from_slice(&width.to_be_bytes());
    header[PNG_IHDR_HEIGHT_OFFSET..PNG_IHDR_HEIGHT_END].copy_from_slice(&height.to_be_bytes());
    header[PNG_BIT_DEPTH_OFFSET] = PNG_GRAYSCALE_BIT_DEPTH;
    append_png_chunk(&mut png, IHDR_CHUNK_TYPE, &header);
    append_png_chunk(&mut png, IDAT_CHUNK_TYPE, &zlib);
    append_png_chunk(&mut png, IEND_CHUNK_TYPE, &[]);
    png
}

fn adler32(bytes: &[u8]) -> u32 {
    let (low, high) = bytes.iter().fold((1_usize, 0_usize), |(low, high), byte| {
        let low = (low + usize::from(*byte)) % ADLER32_MODULUS;
        (low, (high + low) % ADLER32_MODULUS)
    });
    ((high as u32) << ADLER32_HIGH_SHIFT) | low as u32
}

fn append_png_chunk(png: &mut Vec<u8>, kind: &[u8; PNG_CHUNK_TYPE_BYTES], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let crc = png_crc32(&png[png.len() - data.len() - PNG_CHUNK_TYPE_BYTES..]);
    png.extend_from_slice(&crc.to_be_bytes());
}

fn png_crc32(bytes: &[u8]) -> u32 {
    bytes.iter().fold(!0_u32, |crc, byte| {
        (0..BITS_PER_BYTE).fold(crc ^ u32::from(*byte), |value, _| {
            if value & 1 == 0 {
                value >> 1
            } else {
                (value >> 1) ^ PNG_CRC32_POLYNOMIAL
            }
        })
    }) ^ !0_u32
}

fn render(html: &str) -> TestResult<String> {
    HtmlRenderer
        .render(&HtmlRenderInput {
            source: html.to_string(),
        })
        .map(|output| output.content)
        .map_err(|error| format!("HTML runtime must render test fixture: {error}"))
}
