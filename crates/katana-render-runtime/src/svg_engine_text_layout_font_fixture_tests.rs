use super::{TestResult, usvg};
use std::sync::Arc;

pub(super) fn fixture_database(
    bytes: &[u8],
) -> TestResult<(Arc<usvg::fontdb::Database>, usvg::fontdb::ID)> {
    let mut database = usvg::fontdb::Database::new();
    let ids = database.load_font_source(usvg::fontdb::Source::Binary(Arc::new(bytes.to_vec())));
    let [face] = ids.as_slice() else {
        return Err("expected one fixture font face".into());
    };
    Ok((Arc::new(database), *face))
}

#[test]
fn fixture_rejects_invalid_font_bytes() {
    assert!(fixture_database(b"not a font").is_err());
}
