use super::{Mode, upstream_writer_font_database, usvg};
use std::path::Path;

pub(super) fn assert_invalid_size(input: &Path) -> Result<(), String> {
    match parse_fixture(input)? {
        Ok(_) => Err(format!("{}: expected InvalidSize", input.display())),
        Err(usvg::Error::InvalidSize) => Ok(()),
        Err(error) => Err(format!(
            "{}: expected InvalidSize, got {error:?}",
            input.display()
        )),
    }
}

pub(super) fn assert_not_utf8(input: &Path) -> Result<(), String> {
    match parse_fixture(input)? {
        Ok(_) => Err(format!("{}: expected NotAnUtf8Str", input.display())),
        Err(usvg::Error::NotAnUtf8Str) => Ok(()),
        Err(error) => Err(format!(
            "{}: expected NotAnUtf8Str, got {error:?}",
            input.display()
        )),
    }
}

pub(super) fn assert_no_size_contract(input: &Path) -> Result<(), String> {
    let source = read_fixture_text(input)?;
    let (implicit_tree, explicit_tree) = parse_no_size_control(input, &source)?;
    assert_no_size_tree_dimensions(&implicit_tree, &explicit_tree);
    assert_same_normal_render(&implicit_tree, &explicit_tree)?;
    Ok(())
}

fn read_fixture_text(input: &Path) -> Result<String, String> {
    let bytes =
        std::fs::read(input).map_err(|error| format!("read {}: {error}", input.display()))?;
    String::from_utf8(bytes).map_err(|error| format!("decode {}: {error}", input.display()))
}

fn parse_no_size_control(input: &Path, source: &str) -> Result<(usvg::Tree, usvg::Tree), String> {
    let explicit_source = explicit_size_source(source)?;
    let options = fixture_options(input)?;
    assert_eq!(options.default_size.width(), 100.0);
    assert_eq!(options.default_size.height(), 100.0);
    let implicit_tree = usvg::Tree::from_str(source, &options)
        .map_err(|error| format!("parse {}: {error}", input.display()))?;
    let explicit_tree = usvg::Tree::from_str(&explicit_source, &options).map_err(|error| {
        format!(
            "parse explicit-size control for {}: {error}",
            input.display()
        )
    })?;
    Ok((implicit_tree, explicit_tree))
}

fn assert_no_size_tree_dimensions(implicit_tree: &usvg::Tree, explicit_tree: &usvg::Tree) {
    assert_eq!(implicit_tree.size().width(), 199.0);
    assert_eq!(implicit_tree.size().height(), 199.0);
    assert_eq!(implicit_tree.size(), explicit_tree.size());
}

fn assert_same_normal_render(
    implicit_tree: &usvg::Tree,
    explicit_tree: &usvg::Tree,
) -> Result<(), String> {
    let implicit_render = super::render(implicit_tree, Mode::Normal)?;
    let explicit_render = super::render(explicit_tree, Mode::Normal)?;
    assert_eq!(implicit_render.width(), explicit_render.width());
    assert_eq!(implicit_render.height(), explicit_render.height());
    assert_eq!(implicit_render.data(), explicit_render.data());
    Ok(())
}

fn parse_fixture(input: &Path) -> Result<Result<usvg::Tree, usvg::Error>, String> {
    let bytes =
        std::fs::read(input).map_err(|error| format!("read {}: {error}", input.display()))?;
    let options = fixture_options(input)?;
    Ok(usvg::Tree::from_data(&bytes, &options))
}

fn fixture_options(input: &Path) -> Result<usvg::Options<'static>, String> {
    Ok(usvg::Options {
        fontdb: upstream_writer_font_database()?,
        resources_dir: input.parent().map(Path::to_path_buf),
        ..usvg::Options::default()
    })
}

fn explicit_size_source(source: &str) -> Result<String, String> {
    let root_end = source.find('>').ok_or("fixture SVG root is missing")?;
    let root = &source[..root_end];
    if !root.starts_with("<svg") {
        return Err("fixture does not start with an SVG root".to_owned());
    }
    Ok(format!(
        "{root} width=\"199\" height=\"199\"{}",
        &source[root_end..]
    ))
}
