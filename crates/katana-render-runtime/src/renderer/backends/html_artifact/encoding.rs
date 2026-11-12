use super::types::HtmlArtifactError;
use serde::Serialize;
use std::io::{self, Write};

pub(super) fn to_vec_bounded<T: Serialize>(
    value: &T,
    limit: u64,
) -> Result<Vec<u8>, HtmlArtifactError> {
    let mut writer = BoundedWriter::new(limit);
    serde_json::to_writer(&mut writer, value).map_err(|error| {
        if error.is_io() {
            HtmlArtifactError::ArtifactTooLarge
        } else {
            HtmlArtifactError::Serialization(error)
        }
    })?;
    Ok(writer.into_inner())
}

struct BoundedWriter {
    bytes: Vec<u8>,
    limit: usize,
}

impl BoundedWriter {
    fn new(limit: u64) -> Self {
        Self {
            bytes: Vec::new(),
            limit: usize::try_from(limit).unwrap_or(usize::MAX),
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let remaining = self.limit.saturating_sub(self.bytes.len());
        if bytes.len() > remaining {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "HTML artifact encoding limit exceeded",
            ));
        }
        let required = self.bytes.len() + bytes.len();
        let capacity = self.bytes.capacity();
        if required > capacity {
            let target = capacity.saturating_mul(2).max(required).min(self.limit);
            self.bytes.reserve_exact(target - self.bytes.len());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::BoundedWriter;
    use super::to_vec_bounded;
    use serde::ser::{Serialize, Serializer};
    use std::io::Write;

    const SERIALIZE_LIMIT: u64 = 128;

    #[test]
    fn bounded_writer_rejects_overflow_without_growing() {
        let mut writer = BoundedWriter::new(3);
        assert!(writer.write_all(b"abc").is_ok());
        assert!(writer.write_all(b"d").is_err());
        assert!(writer.flush().is_ok());
        assert_eq!(writer.into_inner(), b"abc");
    }

    #[test]
    fn bounded_writer_grows_in_bounded_steps_for_small_writes() {
        let mut writer = BoundedWriter::new(17);
        assert!(writer.write(&[]).is_ok());
        assert_eq!(writer.bytes.capacity(), 0);
        for byte in b"small writes" {
            let capacity = writer.bytes.capacity();
            let has_spare_capacity = writer.bytes.len() < capacity;
            assert!(writer.write_all(std::slice::from_ref(byte)).is_ok());
            if has_spare_capacity {
                assert_eq!(writer.bytes.capacity(), capacity);
            }
            assert!(writer.bytes.capacity() <= 17);
        }
        assert_eq!(writer.into_inner(), b"small writes");
    }

    #[test]
    fn bounded_writer_reserves_from_length_when_capacity_has_slack() {
        let mut writer = BoundedWriter::new(11);
        writer.bytes.reserve_exact(10);
        writer.bytes.extend_from_slice(b"12345678");

        assert!(writer.write_all(b"xyz").is_ok());
        assert!(writer.bytes.capacity() <= 11);
        assert_eq!(writer.into_inner(), b"12345678xyz");
    }

    struct FailingSerialize;

    impl Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(serde::ser::Error::custom("synthetic serializer failure"))
        }
    }

    #[test]
    fn bounded_serializer_preserves_non_io_errors() {
        assert!(matches!(
            to_vec_bounded(&FailingSerialize, SERIALIZE_LIMIT),
            Err(super::HtmlArtifactError::Serialization(_))
        ));
    }
}
