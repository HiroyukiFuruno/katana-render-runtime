use miniz_oxide::MZFlush;
use miniz_oxide::MZStatus;
use miniz_oxide::StreamResult;
use miniz_oxide::inflate::stream::InflateState;
use miniz_oxide::inflate::stream::inflate;
use std::fs;
use std::io::{self, Read, Seek, Write};
use std::path::Path;

const DECOMPRESSION_BUFFER_SIZE: usize = 16 * 1024;
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

pub(super) fn decompress_to_writer<R, W>(
  input: &mut R,
  output: &mut W,
) -> io::Result<()>
where
  R: Read,
  W: Write,
{
  gzip_header::read_gz_header(input)?;
  decompress_gzip_payload(input, output)
}

pub(super) fn decompress_gzip_payload<R, W>(
  input: &mut R,
  output: &mut W,
) -> io::Result<()>
where
  R: Read,
  W: Write,
{
  let mut inflate_state = InflateState::default();
  let mut input_buffer = [0; DECOMPRESSION_BUFFER_SIZE];
  let mut output_buffer = [0; DECOMPRESSION_BUFFER_SIZE];
  let mut input_offset = 0;

  loop {
    if inflate_gzip_chunk(
      input,
      output,
      &mut inflate_state,
      &mut input_buffer,
      &mut output_buffer,
      &mut input_offset,
    )? {
      break;
    }
  }

  Ok(())
}

pub(super) fn inflate_gzip_chunk<R, W>(
  input: &mut R,
  output: &mut W,
  state: &mut InflateState,
  input_buffer: &mut [u8],
  output_buffer: &mut [u8],
  input_offset: &mut usize,
) -> io::Result<bool>
where
  R: Read,
  W: Write,
{
  let bytes_read = input.read(&mut input_buffer[*input_offset..])?;
  let bytes_avail = *input_offset + bytes_read;
  let StreamResult {
    bytes_consumed,
    bytes_written,
    status,
  } = inflate(
    state,
    &input_buffer[..bytes_avail],
    output_buffer,
    MZFlush::None,
  );
  if status != Ok(MZStatus::Ok) && status != Ok(MZStatus::StreamEnd) {
    return Err(io::Error::other(format!("Decompression error {status:?}")));
  }
  output.write_all(&output_buffer[..bytes_written])?;
  /* Move remaining bytes to the beginning of the buffer. */
  input_buffer.copy_within(bytes_consumed..bytes_avail, 0);
  *input_offset = bytes_avail - bytes_consumed;
  Ok(status == Ok(MZStatus::StreamEnd))
}

/// Copy the V8 archive at `url` to `filename`.
///
/// This function doesn't use [`fs::copy`] because that would
/// preserve the file attributes such as ownership and mode flags.
/// Instead, it copies the file contents to a new file.
/// This is necessary because the V8 archive could live inside a read-only
/// filesystem, and subsequent builds would fail to overwrite it.
pub(super) fn copy_archive(url: &str, filename: &Path) -> Result<(), String> {
  println!("Copying {url} to {filename:?}");
  /* Write to a scratch file and rename into place on success, so a source */
  /* that fails partway through cannot truncate a previously good artifact. */
  let partfile = filename.with_extension("part");
  if let Err(error) = copy_archive_to(url, &partfile) {
    let _ = fs::remove_file(&partfile);
    return Err(error);
  }
  fs::rename(&partfile, filename).map_err(|error| {
    let _ = fs::remove_file(&partfile);
    format!(
      "failed to rename {} to {}: {error}",
      partfile.display(),
      filename.display()
    )
  })
}

pub(super) fn copy_archive_to(
  url: &str,
  dst_path: &Path,
) -> Result<(), String> {
  let mut src = fs::File::open(url)
    .map_err(|e| format!("failed to open source archive {url}: {e}"))?;
  let mut dst = fs::File::create(dst_path)
    .map_err(|e| format!("failed to create {}: {e}", dst_path.display()))?;

  /* Allow both GZIP and non-GZIP downloads */
  let mut header = [0; 2];
  src
    .read_exact(&mut header)
    .map_err(|e| format!("failed to read source archive {url}: {e}"))?;
  src
    .seek(io::SeekFrom::Start(0))
    .map_err(|e| format!("failed to seek source archive {url}: {e}"))?;
  if header == GZIP_MAGIC {
    println!("Detected GZIP archive: {url}");
    decompress_to_writer(&mut src, &mut dst).map_err(|e| {
      format!(
        "failed to decompress {url} into {}: {e}",
        dst_path.display()
      )
    })?;
  } else {
    println!("Not a GZIP archive: {url}");
    io::copy(&mut src, &mut dst).map_err(|e| {
      format!("failed to copy {url} to {}: {e}", dst_path.display())
    })?;
  }
  Ok(())
}
