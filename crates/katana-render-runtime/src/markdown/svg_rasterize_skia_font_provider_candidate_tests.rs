use super::super::font::html_font_db_for_text;
use resvg::usvg;
use resvg::usvg::fontdb::{Database, Family, Query, Source, Style, Weight};
use skia_safe::{Data, FontMgr, FontStyle, Typeface, textlayout::TypefaceFontProvider};

const FAMILY: &str = "Trebuchet MS";
const ALIAS: &str = "KrrProbeTrebuchetAlias";
const WIDTH: u32 = 160;
const HEIGHT: u32 = 64;
const RGBA_CHANNELS: usize = 4;
const ALPHA_CHANNEL: usize = 3;
const TEXT_BAND_COUNT: usize = 2;

type ProviderPixelVariants = (Vec<u8>, Vec<u8>, Vec<u8>);

#[test]
fn provider_binds_selected_trebuchet_faces_and_preserves_ordered_fallback() -> Result<(), String> {
    let database = html_font_db_for_text(FAMILY, "Aa");
    let normal = selected_face(&database, Weight::NORMAL)?;
    let bold = selected_face(&database, Weight::BOLD)?;
    let normal_typeface = selected_typeface(&database, normal)?;
    let bold_typeface = selected_typeface(&database, bold)?;
    assert_registered_styles(&normal_typeface, &bold_typeface)?;
    let (provider_pixels, ordered_pixels, default_pixels) = render_variants(
        &normal_typeface,
        &bold_typeface,
        usvg::Size::from_wh(WIDTH as f32, HEIGHT as f32).ok_or("invalid probe size")?,
    )?;
    assert_text_bands(&provider_pixels)?;
    assert_text_bands(&ordered_pixels)?;
    if provider_pixels != ordered_pixels {
        return Err("ordered system fallback changed controlled provider pixels".to_string());
    }
    eprintln!(
        "skia provider probe family={FAMILY} normal_id={} bold_id={} default_delta={}",
        normal_typeface.unique_id(),
        bold_typeface.unique_id(),
        default_pixels != provider_pixels
    );
    Ok(())
}

fn selected_face(database: &Database, weight: Weight) -> Result<resvg::usvg::fontdb::ID, String> {
    let id = database
        .query(&Query {
            families: &[Family::Name(FAMILY)],
            weight,
            style: Style::Normal,
            ..Query::default()
        })
        .ok_or_else(|| format!("selected {FAMILY} weight {} face missing", weight.0))?;
    let face = database.face(id).ok_or("selected face disappeared")?;
    if face.weight != weight || face.style != Style::Normal {
        return Err("selected face differs from requested physical style".to_string());
    }
    Ok(id)
}

fn assert_text_bands(pixels: &[u8]) -> Result<(), String> {
    let expected = WIDTH as usize * HEIGHT as usize * RGBA_CHANNELS;
    if pixels.len() != expected {
        return Err("controlled provider canvas size differs".to_string());
    }
    for band in pixels.chunks_exact(expected / TEXT_BAND_COUNT) {
        if !band
            .chunks_exact(RGBA_CHANNELS)
            .any(|pixel| pixel[ALPHA_CHANNEL] > 0)
        {
            return Err("controlled provider text band is transparent".to_string());
        }
    }
    Ok(())
}

fn selected_typeface(database: &Database, id: resvg::usvg::fontdb::ID) -> Result<Typeface, String> {
    let Some((source, index)) = database.face_source(id) else {
        return Err("selected face has no physical source".to_string());
    };
    if !matches!(source, Source::File(_) | Source::SharedFile(_, _)) {
        return Err("selected face is not file-backed".to_string());
    }
    let (bytes, actual_index) = database
        .with_face_data(id, |bytes, face_index| (bytes.to_vec(), face_index))
        .ok_or_else(|| "selected face bytes unavailable".to_string())?;
    if actual_index != index {
        return Err("selected face index changed during byte read".to_string());
    }
    let typeface = FontMgr::default()
        .new_from_data(Data::new_copy(&bytes), Some(index))
        .ok_or_else(|| "Skia could not create selected physical typeface".to_string())?;
    let Some((cached, cached_index)) = typeface.to_existing_font_data() else {
        return Err("selected physical typeface has no source data".to_string());
    };
    if cached_index != index || cached.as_bytes() != bytes.as_slice() {
        return Err("Skia Typeface source bytes/index differ from selected face".to_string());
    }
    Ok(typeface)
}

fn assert_registered_styles(normal: &Typeface, bold: &Typeface) -> Result<(), String> {
    let provider = provider_for(normal, bold);
    let normal_match = provider
        .match_family_style(ALIAS, FontStyle::normal())
        .ok_or_else(|| "provider normal style missing".to_string())?;
    let bold_match = provider
        .match_family_style(ALIAS, FontStyle::bold())
        .ok_or_else(|| "provider bold style missing".to_string())?;
    if !Typeface::equal(&normal_match, normal) || !Typeface::equal(&bold_match, bold) {
        return Err("provider style match did not preserve selected typefaces".to_string());
    }
    Ok(())
}

fn render_variants(
    normal: &Typeface,
    bold: &Typeface,
    size: usvg::Size,
) -> Result<ProviderPixelVariants, String> {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"160\" height=\"64\"><text x=\"4\" y=\"28\" font-family=\"{ALIAS}\" font-size=\"20\">Aa</text><text x=\"4\" y=\"56\" font-family=\"{ALIAS}\" font-size=\"20\" font-weight=\"700\">Aa</text></svg>"
    );
    let default_pixels =
        super::render_inner_with_font_mgr(&svg, size, 1.0, false, FontMgr::default())
            .map_err(|error| error.to_string())?
            .rgba;
    let provider = provider_for(normal, bold);
    let provider_pixels = super::render_inner_with_font_mgr(&svg, size, 1.0, false, provider)
        .map_err(|error| error.to_string())?
        .rgba;
    let mut ordered = skia_safe::utils::OrderedFontMgr::new();
    ordered.append(provider_for(normal, bold));
    ordered.append(FontMgr::default());
    let ordered_pixels = super::render_inner_with_font_mgr(&svg, size, 1.0, false, ordered.into())
        .map_err(|error| error.to_string())?
        .rgba;
    Ok((provider_pixels, ordered_pixels, default_pixels))
}

fn provider_for(normal: &Typeface, bold: &Typeface) -> FontMgr {
    let mut provider = TypefaceFontProvider::new();
    provider.register_typeface(normal.clone(), Some(ALIAS));
    provider.register_typeface(bold.clone(), Some(ALIAS));
    provider.into()
}
