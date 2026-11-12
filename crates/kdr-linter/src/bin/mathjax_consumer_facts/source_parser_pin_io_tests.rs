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

#[test]
fn parser_pin_read_rejects_lock_without_package_entries() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = Fixture::copy_mathjax_inputs(INPUT_PATHS)?;
    fs::write(fixture.root().join("Cargo.lock"), "[metadata]\n")?;
    let error = fixture.inspect_parser_pin().err().ok_or("lock accepted")?;
    assert_eq!(error.to_string(), "parser-pin:lock-packages");
    Ok(())
}

fn assert_file_io_errors(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_mathjax_inputs(INPUT_PATHS)?;
    let file = fixture.root().join(name);
    fs::remove_file(&file)?;
    assert_parser_pin_io_error(&fixture, name, "missing-source");
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
    assert_parser_pin_io_error(&fixture, name, "read-source");
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

fn assert_parser_pin_io_error(fixture: &Fixture, name: &str, kind: &str) {
    let expected = format!("{kind}:{name}:");
    let result = super::super::parser_pin(fixture.root(), LOCK, WORKSPACE, LINTER);
    assert!(
        matches!(&result, Err(error) if error.starts_with(&expected)),
        "{result:?}"
    );
}
