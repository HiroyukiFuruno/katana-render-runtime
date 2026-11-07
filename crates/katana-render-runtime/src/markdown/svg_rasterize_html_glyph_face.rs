use super::super::physical_font_face as ttc;
use super::types::{CachedTypeface, FaceMetadata};
use super::{CachedFace, HTML_FONT_MANAGER, HtmlGlyphRasterizer};
use resvg::usvg;
use skia_safe::Data;
use skrifa::{FontRef, MetadataProvider, raw::TableProvider};
use std::path::Path;

impl HtmlGlyphRasterizer<'_> {
    pub(super) fn ensure_face(&self, id: usvg::fontdb::ID) -> Result<Option<CachedFace>, String> {
        if let Some(face) = self.faces.borrow().get(&id) {
            return Ok(face.clone());
        }
        let Some((path, index)) = self.face_file(id)? else {
            self.faces.borrow_mut().insert(id, None);
            return Ok(None);
        };
        let metadata = self.face_metadata(id)?;
        if metadata.unsupported {
            self.faces.borrow_mut().insert(id, None);
            return Ok(None);
        }
        let data = self.map_face_data(id, &path)?;
        let Some(typeface) = self.create_backend_face(&data, index)? else {
            self.faces.borrow_mut().insert(id, None);
            return Ok(None);
        };
        let face = CachedFace {
            typeface,
            upem: metadata.upem,
            ascent: metadata.ascent,
            descent: metadata.descent,
            x_height: metadata.x_height,
        };
        self.faces.borrow_mut().insert(id, Some(face.clone()));
        Ok(Some(face))
    }

    pub(super) fn create_backend_face(
        &self,
        data: &Data,
        index: u32,
    ) -> Result<Option<CachedTypeface>, String> {
        let font_mgr = self
            .font_mgr
            .get_or_init(|| HTML_FONT_MANAGER.with(Clone::clone));
        let Some(typeface) = ttc::create_physical_typeface(data, index, font_mgr)? else {
            return Ok(None);
        };
        Ok(Some(CachedTypeface::Skia(typeface)))
    }

    pub(super) fn face_file(
        &self,
        id: usvg::fontdb::ID,
    ) -> Result<Option<(std::path::PathBuf, u32)>, String> {
        let Some((source, index)) = self.database.face_source(id) else {
            return Err(format!("SVG font face {id:?} is unavailable"));
        };
        match source {
            usvg::fontdb::Source::File(path) | usvg::fontdb::Source::SharedFile(path, _) => {
                Ok(Some((path, index)))
            }
            usvg::fontdb::Source::Binary(_) => Ok(None),
        }
    }

    pub(super) fn face_metadata(&self, id: usvg::fontdb::ID) -> Result<FaceMetadata, String> {
        self.face_metadata_at(id, None)
    }

    pub(super) fn face_metadata_at(
        &self,
        id: usvg::fontdb::ID,
        variations: Option<&[usvg::FontVariation]>,
    ) -> Result<FaceMetadata, String> {
        self.database
            .with_face_data(id, |bytes, index| {
                parsed_face_metadata(bytes, index, variations)
            })
            .flatten()
            .ok_or_else(|| format!("SVG font face {id:?} cannot be parsed"))
    }

    pub(super) fn map_face_data(&self, id: usvg::fontdb::ID, path: &Path) -> Result<Data, String> {
        let Some(data) = Data::from_filename(path) else {
            return Err(format!("SVG font file {} cannot be mapped", path.display()));
        };
        let Some(same_bytes) = self
            .database
            .with_face_data(id, |bytes, _| bytes == data.as_bytes())
        else {
            return Err(format!("SVG font face {id:?} data is unavailable"));
        };
        if !same_bytes {
            return Err(format!(
                "SVG font file {} differs from parsed face",
                path.display()
            ));
        }
        Ok(data)
    }
}

fn parsed_face_metadata(
    bytes: &[u8],
    index: u32,
    variations: Option<&[usvg::FontVariation]>,
) -> Option<FaceMetadata> {
    let font = FontRef::from_index(bytes, index).ok()?;
    let metrics = font_metrics_at(&font, variations);
    let variable = !font.axes().is_empty();
    let color_or_bitmap =
        font.colr().is_ok() || font.svg().is_ok() || !font.bitmap_strikes().is_empty();
    Some(FaceMetadata {
        upem: font.head().ok()?.units_per_em() as f32,
        unsupported: (variable && variations.is_none()) || color_or_bitmap,
        ascent: metrics.ascent.round(),
        descent: metrics.descent.round(),
        x_height: metrics.x_height,
    })
}

fn font_metrics_at(
    font: &FontRef<'_>,
    variations: Option<&[usvg::FontVariation]>,
) -> skrifa::metrics::Metrics {
    let location = match variations {
        Some(axes) => font.axes().location(
            axes.iter()
                .map(|axis| (skrifa::Tag::from_be_bytes(axis.tag), axis.value)),
        ),
        None => font.axes().location(std::iter::empty::<(&str, f32)>()),
    };
    font.metrics(skrifa::prelude::Size::unscaled(), &location)
}
