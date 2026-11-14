use super::{CachedFace, CachedTypeface, HtmlGlyphRasterizer};
use crate::markdown::svg_rasterize::html_glyph_variations;
use resvg::usvg::{self, FontVariation, fontdb::ID};
use skia_safe::Typeface;
use std::{collections::HashMap, path::PathBuf, rc::Rc};

#[path = "svg_rasterize_html_glyph_instance_typeface.rs"]
mod typeface;
use typeface::instantiate;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct InstanceKey {
    font: ID,
    axes: Vec<(skrifa::Tag, u32)>,
}

impl InstanceKey {
    fn new(font: ID, axes: &[FontVariation]) -> Result<Self, String> {
        html_glyph_variations::validate_variations(font, axes)?;
        let mut axes = axes
            .iter()
            .map(|axis| {
                let value = if axis.value == 0.0 { 0.0 } else { axis.value };
                (skrifa::Tag::from_be_bytes(axis.tag), value.to_bits())
            })
            .collect::<Vec<_>>();
        axes.sort_unstable();
        Ok(Self { font, axes })
    }
}

pub(super) struct VariableInstance {
    pub(super) face: CachedFace,
    #[cfg(test)]
    pub(super) applied_axes: Vec<FontVariation>,
}

#[derive(Default)]
pub(super) struct Cache {
    bases: HashMap<ID, CachedTypeface>,
    instances: HashMap<InstanceKey, Rc<VariableInstance>>,
}

type RunFace = (CachedFace, Option<Rc<VariableInstance>>);

impl HtmlGlyphRasterizer<'_> {
    pub(super) fn ensure_run_face(
        &self,
        span: &usvg::layout::Span,
        id: ID,
        font_size: f32,
    ) -> Result<Option<RunFace>, String> {
        if let Some(face) = self.ensure_face(id)? {
            return Ok(Some((face, None)));
        }
        let Some(instance) = self.variable_instance(span, id, font_size)? else {
            return Ok(None);
        };
        Ok(Some((instance.face.clone(), Some(instance))))
    }

    fn variable_instance(
        &self,
        span: &usvg::layout::Span,
        id: ID,
        font_size: f32,
    ) -> Result<Option<Rc<VariableInstance>>, String> {
        let Some(file) = self.face_file(id)? else {
            return Ok(None);
        };
        let (_, design) = html_glyph_variations::resolve_design(
            self.database,
            id,
            &span.variations,
            span.font_optical_sizing,
            font_size,
        )?;
        if design.is_empty() {
            return Ok(None);
        }
        let requested_key = InstanceKey::new(id, &design)?;
        if let Some(instance) = self.variable_faces.borrow().instances.get(&requested_key) {
            return Ok(Some(instance.clone()));
        }
        self.create_variable_instance(id, &design, requested_key, file)
    }

    fn create_variable_instance(
        &self,
        id: ID,
        design: &[FontVariation],
        requested_key: InstanceKey,
        file: (PathBuf, u32),
    ) -> Result<Option<Rc<VariableInstance>>, String> {
        if self.face_metadata_at(id, Some(design))?.unsupported {
            return Ok(None);
        }
        let Some(base) = self.variable_base(id, file)? else {
            return Ok(None);
        };
        let (typeface, applied_axes) = instantiate(base.as_skia(), design)?;
        let actual_key = InstanceKey::new(id, &applied_axes)?;
        /* WHY: 要求値を適用済みと見なさず、実Typefaceの全axisをcacheとmetricsへ束縛する。 */
        if actual_key != requested_key {
            return Err(format!(
                "Skia applied variation coordinates differ for face {id:?}"
            ));
        }
        self.cache_variable_instance(id, typeface, applied_axes, actual_key)
            .map(Some)
    }

    fn cache_variable_instance(
        &self,
        id: ID,
        typeface: Typeface,
        applied_axes: Vec<FontVariation>,
        actual_key: InstanceKey,
    ) -> Result<Rc<VariableInstance>, String> {
        let metrics = self.face_metadata_at(id, Some(&applied_axes))?;
        let instance = Rc::new(VariableInstance {
            face: CachedFace {
                typeface: CachedTypeface::Skia(typeface),
                upem: metrics.upem,
                ascent: metrics.ascent,
                descent: metrics.descent,
                x_height: metrics.x_height,
            },
            #[cfg(test)]
            applied_axes,
        });
        self.variable_faces
            .borrow_mut()
            .instances
            .insert(actual_key, instance.clone());
        Ok(instance)
    }

    fn variable_base(
        &self,
        id: ID,
        (path, index): (PathBuf, u32),
    ) -> Result<Option<CachedTypeface>, String> {
        if let Some(base) = self.variable_faces.borrow().bases.get(&id) {
            return Ok(Some(base.clone()));
        }
        let Some(base) = self.load_file_typeface(id, &path, index)? else {
            return Ok(None);
        };
        self.variable_faces
            .borrow_mut()
            .bases
            .insert(id, base.clone());
        Ok(Some(base))
    }

    pub(super) fn primary_variable_face(
        &self,
        text: &usvg::Text,
        id: ID,
    ) -> Result<Option<CachedFace>, String> {
        let mut selected: Option<Rc<VariableInstance>> = None;
        for span in text.layouted() {
            for run in
                super::runs::consecutive_runs(&span.positioned_glyphs).filter(|run| run.font == id)
            {
                let Some(instance) = self.variable_instance(span, id, run.font_size)? else {
                    return Ok(None);
                };
                if selected
                    .as_ref()
                    .is_some_and(|old| !Rc::ptr_eq(old, &instance))
                {
                    return Ok(None);
                }
                selected = Some(instance);
            }
        }
        Ok(selected.map(|instance| instance.face.clone()))
    }
}

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_instances_tests.rs"]
mod tests;
