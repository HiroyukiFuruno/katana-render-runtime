use super::{FONT, database_with_font, usvg};

pub(super) fn path_text(rotation: u32) -> Result<usvg::Tree, String> {
    path_text_with_content(rotation, "H")
}

pub(super) fn path_text_with_content(rotation: u32, content: &str) -> Result<usvg::Tree, String> {
    let (fontdb, _) = database_with_font(FONT.to_vec()).map_err(|error| error.to_string())?;
    let options = usvg::Options {
        fontdb,
        ..usvg::Options::default()
    };
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='200' height='100'><defs><path id='route' d='M0 60 L180 60'/></defs><text id='sample' font-family='Noto Sans' font-size='20' rotate='{rotation}'><textPath href='#route'>{content}</textPath></text></svg>"
    );
    usvg::Tree::from_str(&svg, &options).map_err(|error| error.to_string())
}
