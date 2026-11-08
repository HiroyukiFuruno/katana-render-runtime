use super::{Fixture, INPUT_PATHS, LINTER, LOCK, WORKSPACE};
use std::fs;
use std::io;

#[test]
fn parser_pin_propagates_fixture_file_io_errors() -> Result<(), Box<dyn std::error::Error>> {
    for name in INPUT_PATHS {
        assert_file_io_errors(name)?;
    }
    Ok(())
}

fn assert_file_io_errors(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_mathjax_inputs(INPUT_PATHS)?;
    let file = fixture.root().join(name);
    fs::remove_file(&file)?;
    let expected = fs::read_to_string(&file)
        .err()
        .ok_or("missing read error")?;
    let actual = fixture
        .inspect_parser_pin()
        .err()
        .ok_or("missing pin read error")?;
    assert_eq!(
        actual.downcast_ref::<io::Error>().map(io::Error::kind),
        Some(io::ErrorKind::NotFound)
    );
    assert_eq!(actual.to_string(), expected.to_string());
    fs::create_dir(&file)?;
    let expected = fs::write(&file, "data")
        .err()
        .ok_or("missing write error")?;
    let actual = fixture
        .inspect_parser_pin_inputs(WORKSPACE, LINTER, LOCK)
        .err()
        .ok_or("missing pin write error")?;
    assert!(actual.is::<io::Error>());
    assert_eq!(actual.to_string(), expected.to_string());
    Ok(())
}
