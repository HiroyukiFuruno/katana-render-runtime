// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

use crate::svg_engine::usvg::{FontVariation, GlyphId};
use fontdb::Database;
use fontdb::ID;
use svgtypes::{Length, LengthUnit as Unit, PaintOrderKind, TransformOrigin};
use tiny_skia_path::PathBuilder;

use super::svgtree::{self, AId, EId, FromValue, SvgNode};
use super::units::{self, convert_length};
use super::{Error, Options, marker};
use crate::svg_engine::usvg::flatten::BitmapImage;
use crate::svg_engine::usvg::parser::paint_server::process_paint;
use crate::svg_engine::usvg::text::flatten::DatabaseExt;
use crate::svg_engine::usvg::*;

#[derive(Clone)]
pub struct State<'a> {
    pub(crate) parent_clip_path: Option<SvgNode<'a, 'a>>,
    pub(crate) parent_markers: Vec<SvgNode<'a, 'a>>,
    /// Stores the resolved fill and stroke of a use node
    /// or a path element (for markers)
    pub(crate) context_element: Option<(Option<Fill>, Option<Stroke>)>,
    pub(crate) fe_image_link: bool,
    /// A viewBox of the parent SVG element.
    pub(crate) view_box: NonZeroRect,
    /// A size of the parent `use` element.
    /// Used only during nested `svg` size resolving.
    /// Width and height can be set independently.
    pub(crate) use_size: (Option<f32>, Option<f32>),
    pub(crate) opt: &'a Options<'a>,
    pub(crate) fontdb: Rc<RefCell<Arc<Database>>>,
}

#[derive(Clone)]
pub struct Cache {
    /// This fontdb is initialized from [`Options::fontdb`] and then populated
    /// over the course of conversion.
    pub fontdb: Arc<Database>,
    pub(crate) shared_fontdb: Rc<RefCell<Arc<Database>>>,

    cache_outline: HashMap<(ID, GlyphId, Vec<FontVariation>), Option<tiny_skia_path::Path>>,
    cache_colr: HashMap<(ID, GlyphId, Vec<FontVariation>), Option<Tree>>,
    cache_svg: HashMap<(ID, GlyphId), Option<Node>>,
    cache_raster: HashMap<(ID, GlyphId), Option<BitmapImage>>,
    cache_has_opsz: HashMap<ID, bool>,

    pub clip_paths: HashMap<String, Arc<ClipPath>>,
    pub masks: HashMap<String, Arc<Mask>>,
    pub filters: HashMap<String, Arc<filter::Filter>>,
    pub paint: HashMap<String, Paint>,

    // used for ID generation
    all_ids: HashSet<u64>,
    linear_gradient_index: usize,
    radial_gradient_index: usize,
    pattern_index: usize,
    clip_path_index: usize,
    mask_index: usize,
    filter_index: usize,
    image_index: usize,
}

macro_rules! font_lookup {
    ($method_name:ident, $cache_map:ident, $font_variant:ident, $return_type:ty) => {
        pub(crate) fn $method_name(&mut self, font: ID, glyph: GlyphId) -> Option<$return_type> {
            let key = (font, glyph);
            match self.$cache_map.get(&key) {
                Some(cache_hit) => cache_hit.clone(),
                None => {
                    let lookup = self.fontdb.$font_variant(font, glyph);
                    self.$cache_map.insert(key, lookup.clone());
                    lookup
                }
            }
        }
    };
}

impl Cache {
    pub(crate) fn new(
        fontdb: Arc<Database>,
        shared_fontdb: Rc<RefCell<Arc<Database>>>,
    ) -> Self {
        Self {
            fontdb,
            shared_fontdb,

            cache_outline: HashMap::new(),
            cache_colr: HashMap::new(),
            cache_svg: HashMap::new(),
            cache_raster: HashMap::new(),
            cache_has_opsz: HashMap::new(),

            clip_paths: HashMap::new(),
            masks: HashMap::new(),
            filters: HashMap::new(),
            paint: HashMap::new(),

            all_ids: HashSet::new(),
            linear_gradient_index: 0,
            radial_gradient_index: 0,
            pattern_index: 0,
            clip_path_index: 0,
            mask_index: 0,
            filter_index: 0,
            image_index: 0,
        }
    }

    pub(crate) fn sync_fontdb_from_shared(&mut self) {
        self.fontdb = self.shared_fontdb.borrow().clone();
    }

    pub(crate) fn sync_fontdb_to_shared(&mut self) {
        *self.shared_fontdb.borrow_mut() = self.fontdb.clone();
    }

    // TODO: macros?
    pub(crate) fn gen_linear_gradient_id(&mut self) -> NonEmptyString {
        loop {
            self.linear_gradient_index += 1;
            let new_id = format!("linearGradient{}", self.linear_gradient_index);
            let new_hash = string_hash(&new_id);
            if !self.all_ids.contains(&new_hash) {
                return NonEmptyString::new(new_id).unwrap();
            }
        }
    }

    pub(crate) fn gen_radial_gradient_id(&mut self) -> NonEmptyString {
        loop {
            self.radial_gradient_index += 1;
            let new_id = format!("radialGradient{}", self.radial_gradient_index);
            let new_hash = string_hash(&new_id);
            if !self.all_ids.contains(&new_hash) {
                return NonEmptyString::new(new_id).unwrap();
            }
        }
    }

    pub(crate) fn gen_pattern_id(&mut self) -> NonEmptyString {
        loop {
            self.pattern_index += 1;
            let new_id = format!("pattern{}", self.pattern_index);
            let new_hash = string_hash(&new_id);
            if !self.all_ids.contains(&new_hash) {
                return NonEmptyString::new(new_id).unwrap();
            }
        }
    }

    pub(crate) fn gen_clip_path_id(&mut self) -> NonEmptyString {
        loop {
            self.clip_path_index += 1;
            let new_id = format!("clipPath{}", self.clip_path_index);
            let new_hash = string_hash(&new_id);
            if !self.all_ids.contains(&new_hash) {
                return NonEmptyString::new(new_id).unwrap();
            }
        }
    }

    pub(crate) fn gen_mask_id(&mut self) -> NonEmptyString {
        loop {
            self.mask_index += 1;
            let new_id = format!("mask{}", self.mask_index);
            let new_hash = string_hash(&new_id);
            if !self.all_ids.contains(&new_hash) {
                return NonEmptyString::new(new_id).unwrap();
            }
        }
    }

    pub(crate) fn gen_filter_id(&mut self) -> NonEmptyString {
        loop {
            self.filter_index += 1;
            let new_id = format!("filter{}", self.filter_index);
            let new_hash = string_hash(&new_id);
            if !self.all_ids.contains(&new_hash) {
                return NonEmptyString::new(new_id).unwrap();
            }
        }
    }

    pub(crate) fn gen_image_id(&mut self) -> NonEmptyString {
        loop {
            self.image_index += 1;
            let new_id = format!("image{}", self.image_index);
            let new_hash = string_hash(&new_id);
            if !self.all_ids.contains(&new_hash) {
                return NonEmptyString::new(new_id).unwrap();
            }
        }
    }

    font_lookup!(fontdb_svg, cache_svg, svg, Node);
    font_lookup!(fontdb_raster, cache_raster, raster, BitmapImage);

    pub(crate) fn fontdb_outline(
        &mut self,
        font: ID,
        glyph: GlyphId,
        variations: &[FontVariation],
    ) -> Option<tiny_skia_path::Path> {
        let key = (font, glyph, variations.to_vec());
        match self.cache_outline.get(&key) {
            Some(cache_hit) => cache_hit.clone(),
            None => {
                let lookup = self.fontdb.outline(font, glyph, variations);
                self.cache_outline.insert(key, lookup.clone());
                lookup
            }
        }
    }

    pub(crate) fn fontdb_colr(
        &mut self,
        font: ID,
        glyph: GlyphId,
        variations: &[FontVariation],
    ) -> Option<Tree> {
        let key = (font, glyph, variations.to_vec());
        match self.cache_colr.get(&key) {
            Some(cache_hit) => cache_hit.clone(),
            None => {
                let lookup = self.fontdb.colr(font, glyph, variations);
                self.cache_colr.insert(key, lookup.clone());
                lookup
            }
        }
    }

    pub(crate) fn has_opsz_axis(&mut self, font: ID) -> bool {
        if let Some(&cached) = self.cache_has_opsz.get(&font) {
            return cached;
        }
        let has_opsz = self.fontdb.has_opsz_axis(font);
        self.cache_has_opsz.insert(font, has_opsz);
        has_opsz
    }
}

// TODO: is there a simpler way?
fn string_hash(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

impl<'a, 'input: 'a> SvgNode<'a, 'input> {
    pub(crate) fn convert_length(
        &self,
        aid: AId,
        object_units: Units,
        state: &State,
        def: Length,
    ) -> f32 {
        units::convert_length(
            self.attribute(aid).unwrap_or(def),
            *self,
            aid,
            object_units,
            state,
        )
    }

    pub fn convert_user_length(&self, aid: AId, state: &State, def: Length) -> f32 {
        self.convert_length(aid, Units::UserSpaceOnUse, state, def)
    }

    pub fn parse_viewbox(&self) -> Option<NonZeroRect> {
        let vb: svgtypes::ViewBox = self.attribute(AId::ViewBox)?;
        NonZeroRect::from_xywh(vb.x as f32, vb.y as f32, vb.w as f32, vb.h as f32)
    }

    pub fn resolve_length(&self, aid: AId, state: &State, def: f32) -> f32 {
        debug_assert!(
            !matches!(aid, AId::BaselineShift | AId::FontSize),
            "{} cannot be resolved via this function",
            aid
        );

        if let Some(n) = self.ancestors().find(|n| n.has_attribute(aid)) {
            if let Some(length) = n.attribute(aid) {
                return units::convert_user_length(length, n, aid, state);
            }
        }

        def
    }

    pub fn resolve_valid_length(
        &self,
        aid: AId,
        state: &State,
        def: f32,
    ) -> Option<NonZeroPositiveF32> {
        let n = self.resolve_length(aid, state, def);
        NonZeroPositiveF32::new(n)
    }

    pub(crate) fn try_convert_length(
        &self,
        aid: AId,
        object_units: Units,
        state: &State,
    ) -> Option<f32> {
        Some(units::convert_length(
            self.attribute(aid)?,
            *self,
            aid,
            object_units,
            state,
        ))
    }

    pub fn has_valid_transform(&self, aid: AId) -> bool {
        // Do not use Node::attribute::<Transform>, because it will always
        // return a valid transform.

        let attr = match self.attribute(aid) {
            Some(attr) => attr,
            None => return true,
        };

        let ts = match svgtypes::Transform::from_str(attr) {
            Ok(v) => v,
            Err(_) => return true,
        };

        let ts = Transform::from_row(
            ts.a as f32,
            ts.b as f32,
            ts.c as f32,
            ts.d as f32,
            ts.e as f32,
            ts.f as f32,
        );
        ts.is_valid()
    }

    pub fn is_visible_element(&self, opt: &crate::svg_engine::usvg::Options) -> bool {
        self.attribute(AId::Display) != Some("none")
            && self.has_valid_transform(AId::Transform)
            && super::switch::is_condition_passed(*self, opt)
    }
}

pub trait SvgColorExt {
    fn split_alpha(self) -> (Color, Opacity);
}

impl SvgColorExt for svgtypes::Color {
    fn split_alpha(self) -> (Color, Opacity) {
        (
            Color::new_rgb(self.red, self.green, self.blue),
            Opacity::new_u8(self.alpha),
        )
    }
}

/// Converts an input `Document` into a `Tree`.
///
/// # Errors
///
/// - If `Document` doesn't have an SVG node - returns an empty tree.
/// - If `Document` doesn't have a valid size - returns `Error::InvalidSize`.
pub(crate) fn convert_doc(svg_doc: &svgtree::Document, opt: &Options) -> Result<Tree, Error> {
    let svg = svg_doc.root_element();
    let shared_fontdb = Rc::new(RefCell::new(opt.fontdb.clone()));
    let (size, restore_viewbox) = resolve_svg_size(&svg, opt, shared_fontdb.clone());
    let size = size?;
    let view_box = svg_view_box(svg, size);
    let background_color = svg_background(svg);
    let mut tree = empty_svg_tree(size, opt);
    if !svg.is_visible_element(opt) {
        return Ok(tree);
    }
    let state = root_state(view_box.rect, opt, shared_fontdb.clone());
    let shared_fontdb = state.fontdb.clone();
    let mut cache = Cache::new(opt.fontdb.clone(), shared_fontdb);
    collect_source_ids(svg_doc, &mut cache);
    convert_svg_root(svg_doc, &state, &mut cache, &mut tree, view_box, background_color);
    finish_svg_tree(&mut tree, cache, restore_viewbox);
    Ok(tree)
}

fn svg_view_box(svg: SvgNode, size: Size) -> ViewBox {
    ViewBox {
        rect: svg.parse_viewbox().unwrap_or_else(|| size.to_non_zero_rect(0.0, 0.0)),
        aspect: svg.attribute(AId::PreserveAspectRatio).unwrap_or_default(),
    }
}

fn svg_background(svg: SvgNode) -> Option<svgtypes::Color> {
    svg.attribute::<&str>(AId::BackgroundColor)
        .and_then(|s| svgtypes::Paint::from_str(s).ok())
        .and_then(|paint| match paint {
            svgtypes::Paint::Color(c) => Some(c),
            _ => None,
        })
}

fn empty_svg_tree(size: Size, opt: &Options) -> Tree {
    Tree {
        size,
        root: Group::empty(),
        linear_gradients: Vec::new(),
        radial_gradients: Vec::new(),
        patterns: Vec::new(),
        clip_paths: Vec::new(),
        masks: Vec::new(),
        filters: Vec::new(),
        fontdb: opt.fontdb.clone(),
    }
}

fn root_state<'a>(view_box: NonZeroRect, opt: &'a Options<'a>, fontdb: Rc<RefCell<Arc<Database>>>) -> State<'a> {
    State {
        parent_clip_path: None,
        context_element: None,
        parent_markers: Vec::new(),
        fe_image_link: false,
        view_box,
        use_size: (None, None),
        opt,
        fontdb,
    }
}

fn collect_source_ids(svg_doc: &svgtree::Document, cache: &mut Cache) {
    for node in svg_doc.descendants() {
        if let Some(tag) = node.tag_name() {
            if matches!(
                tag,
                EId::ClipPath
                    | EId::Filter
                    | EId::LinearGradient
                    | EId::Mask
                    | EId::Pattern
                    | EId::RadialGradient
                    | EId::Image
            ) {
                if !node.element_id().is_empty() {
                    cache.all_ids.insert(string_hash(node.element_id()));
                }
            }
        }
    }
}

fn convert_svg_root(
    svg_doc: &svgtree::Document,
    state: &State,
    cache: &mut Cache,
    tree: &mut Tree,
    view_box: ViewBox,
    background_color: Option<svgtypes::Color>,
) {
    let root_state = State {
        view_box: tree.size.to_non_zero_rect(0.0, 0.0),
        ..state.clone()
    };
    let svg_ts = svg_doc.root_element().resolve_transform(AId::Transform, &root_state);
    let root_ts = svg_ts.pre_concat(view_box.to_transform(tree.size()));
    if root_ts.is_identity() && background_color.is_none() {
        convert_children(svg_doc.root(), state, cache, &mut tree.root);
    } else {
        append_svg_root_group(svg_doc, state, cache, tree, view_box, background_color, root_ts);
    }
}

fn append_svg_root_group(
    svg_doc: &svgtree::Document,
    state: &State,
    cache: &mut Cache,
    tree: &mut Tree,
    view_box: ViewBox,
    background_color: Option<svgtypes::Color>,
    root_ts: Transform,
) {
    let mut g = Group::empty();
    if let Some(background_color) = background_color {
        if let Some(path) = background_path(background_color, view_box.rect.to_rect()) {
            g.children.push(Node::Path(Box::new(path)));
        }
    }
    g.transform = root_ts;
    g.abs_transform = root_ts;
    convert_children(svg_doc.root(), state, cache, &mut g);
    g.calculate_bounding_boxes();
    tree.root.children.push(Node::Group(Box::new(g)));
}

fn finish_svg_tree(tree: &mut Tree, mut cache: Cache, restore_viewbox: bool) {
    cache.clip_paths.clear();
    cache.masks.clear();
    cache.filters.clear();
    cache.paint.clear();
    super::paint_server::update_paint_servers(
        &mut tree.root,
        Transform::default(),
        None,
        None,
        &mut cache,
    );
    tree.collect_paint_servers();
    tree.root.collect_clip_paths(&mut tree.clip_paths);
    tree.root.collect_masks(&mut tree.masks);
    tree.root.collect_filters(&mut tree.filters);
    tree.root.calculate_bounding_boxes();
    tree.fontdb = cache.fontdb;
    if restore_viewbox {
        calculate_svg_bbox(tree);
    }
}

fn background_path(background_color: svgtypes::Color, area: Rect) -> Option<Path> {
    let path = PathBuilder::from_rect(area);

    let fill = Fill {
        paint: Paint::Color(Color::new_rgb(
            background_color.red,
            background_color.green,
            background_color.blue,
        )),
        opacity: NormalizedF32::new(background_color.alpha as f32 / 255.0)?,
        ..Default::default()
    };

    let mut path = Path::new_simple(Arc::new(path))?;
    path.fill = Some(fill);

    Some(path)
}

fn resolve_svg_size(
    svg: &SvgNode,
    opt: &Options,
    shared_fontdb: Rc<RefCell<Arc<Database>>>,
) -> (Result<Size, Error>, bool) {
    let default_view_box = NonZeroRect::from_xywh(0.0, 0.0, 100.0, 100.0).unwrap();
    let mut state = root_state(default_view_box, opt, shared_fontdb);
    let def = Length::new(100.0, Unit::Percent);
    let mut width: Length = svg.attribute(AId::Width).unwrap_or(def);
    let mut height: Length = svg.attribute(AId::Height).unwrap_or(def);
    let view_box = svg.parse_viewbox();
    let restore_viewbox = resolve_fallback_size(&mut width, &mut height, view_box, &state);
    let size = if let Some(vbox) = view_box {
        state.view_box = vbox;
        size_with_view_box(svg, &state, vbox, width, height, def)
    } else {
        Size::from_wh(
            svg.convert_user_length(AId::Width, &state, def),
            svg.convert_user_length(AId::Height, &state, def),
        )
    };
    (size.ok_or(Error::InvalidSize), restore_viewbox)
}

fn resolve_fallback_size(
    width: &mut Length,
    height: &mut Length,
    view_box: Option<NonZeroRect>,
    state: &State,
) -> bool {
    if (width.unit == Unit::Percent || height.unit == Unit::Percent) && view_box.is_none() {
        if width.unit == Unit::Percent {
            *width = Length::new(
                (width.number / 100.0) * state.opt.default_size.width() as f64,
                Unit::None,
            );
        }
        if height.unit == Unit::Percent {
            *height = Length::new(
                (height.number / 100.0) * state.opt.default_size.height() as f64,
                Unit::None,
            );
        }
        true
    } else {
        false
    }
}

fn size_with_view_box(
    svg: &SvgNode,
    state: &State,
    vbox: NonZeroRect,
    width: Length,
    height: Length,
    def: Length,
) -> Option<Size> {
    let w = if width.unit == Unit::Percent {
        vbox.width() * (width.number as f32 / 100.0)
    } else {
        svg.convert_user_length(AId::Width, state, def)
    };
    let h = if height.unit == Unit::Percent {
        vbox.height() * (height.number as f32 / 100.0)
    } else {
        svg.convert_user_length(AId::Height, state, def)
    };
    size_from_view_box(svg, vbox, w, h)
}

fn size_from_view_box(svg: &SvgNode, vbox: NonZeroRect, w: f32, h: f32) -> Option<Size> {
    match (
        svg.attribute::<Length>(AId::Width),
        svg.attribute::<Length>(AId::Height),
    ) {
        (Some(_), None) => Size::from_wh(w, vbox.height() * w / vbox.width()),
        (None, Some(_)) => Size::from_wh(vbox.width() * h / vbox.height(), h),
        (_, _) => Size::from_wh(w, h),
    }
}

/// Calculates SVG's size and viewBox in case there were not set.
///
/// Simply iterates over all nodes and calculates a bounding box.
fn calculate_svg_bbox(tree: &mut Tree) {
    let bbox = tree.root.abs_bounding_box();
    if let Some(size) = Size::from_wh(bbox.right(), bbox.bottom()) {
        tree.size = size;
    }
}

#[inline(never)]
pub(crate) fn convert_children(
    parent_node: SvgNode,
    state: &State,
    cache: &mut Cache,
    parent: &mut Group,
) {
    for node in parent_node.children() {
        convert_element(node, state, cache, parent);
    }
}

enum ElementRoute {
    Use,
    Switch,
    NestedSvg,
    Grouped(GroupedElement),
}

enum GroupedElement {
    Shape,
    Image,
    Text,
    Children,
}

fn element_route(node: SvgNode) -> Option<ElementRoute> {
    let child = match node.tag_name()? {
        EId::Use => return Some(ElementRoute::Use),
        EId::Switch => return Some(ElementRoute::Switch),
        // 入れ子のSVGは自身でグループを作り、変換を一度だけ適用する。
        EId::Svg if node.parent_element().is_some() => return Some(ElementRoute::NestedSvg),
        EId::Rect | EId::Circle | EId::Ellipse | EId::Line
        | EId::Polyline | EId::Polygon | EId::Path => GroupedElement::Shape,
        EId::Image => GroupedElement::Image,
        EId::Text => GroupedElement::Text,
        EId::Svg | EId::G => GroupedElement::Children,
        _ => return None,
    };
    Some(ElementRoute::Grouped(child))
}

#[inline(never)]
pub(crate) fn convert_element(node: SvgNode, state: &State, cache: &mut Cache, parent: &mut Group) {
    let Some(route) = element_route(node) else {
        return;
    };
    if !node.is_visible_element(state.opt) {
        return;
    }

    match route {
        ElementRoute::Use => super::use_node::convert(node, state, cache, parent),
        ElementRoute::Switch => {
            super::switch::convert(node, state, cache, parent);
        }
        ElementRoute::NestedSvg => super::use_node::convert_svg(node, state, cache, parent),
        ElementRoute::Grouped(child) => convert_grouped_element(child, node, state, cache, parent),
    }
}

fn convert_grouped_element(
    child: GroupedElement,
    node: SvgNode,
    state: &State,
    cache: &mut Cache,
    parent: &mut Group,
) {
    if let Some(g) = convert_group(node, state, false, cache, parent, &|cache, g| match child {
        GroupedElement::Shape => convert_shape(node, state, cache, g),
        GroupedElement::Image => {
            super::image::convert(node, state, cache, g);
        }
        GroupedElement::Text => super::text::convert(node, state, cache, g),
        GroupedElement::Children => convert_children(node, state, cache, g),
    }) {
        parent.children.push(Node::Group(Box::new(g)));
    }
}

fn convert_shape(node: SvgNode, state: &State, cache: &mut Cache, parent: &mut Group) {
    if let Some(path) = super::shapes::convert(node, state) {
        convert_path(node, path, state, cache, parent);
    }
}

// `clipPath` can have only shape and `text` children.
//
// `line` doesn't impact rendering because stroke is always disabled
// for `clipPath` children.
#[inline(never)]
pub(crate) fn convert_clip_path_elements(
    clip_node: SvgNode,
    state: &State,
    cache: &mut Cache,
    parent: &mut Group,
) {
    for node in clip_node.children() {
        let tag_name = match node.tag_name() {
            Some(v) => v,
            None => continue,
        };

        if !tag_name.is_graphic() {
            continue;
        }

        if !node.is_visible_element(state.opt) {
            continue;
        }

        if tag_name == EId::Use {
            super::use_node::convert(node, state, cache, parent);
            continue;
        }

        if let Some(g) = convert_group(node, state, false, cache, parent, &|cache, g| {
            convert_clip_path_elements_impl(tag_name, node, state, cache, g);
        }) {
            parent.children.push(Node::Group(Box::new(g)));
        }
    }
}

#[inline(never)]
fn convert_clip_path_elements_impl(
    tag_name: EId,
    node: SvgNode,
    state: &State,
    cache: &mut Cache,
    parent: &mut Group,
) {
    match tag_name {
        EId::Rect | EId::Circle | EId::Ellipse | EId::Polyline | EId::Polygon | EId::Path => {
            if let Some(path) = super::shapes::convert(node, state) {
                convert_path(node, path, state, cache, parent);
            }
        }
        EId::Text => {
            {
                super::text::convert(node, state, cache, parent);
            }
        }
        _ => {
            log::warn!("'{}' is no a valid 'clip-path' child.", tag_name);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Isolation {
    Auto,
    Isolate,
}

impl Default for Isolation {
    fn default() -> Self {
        Self::Auto
    }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for Isolation {
    fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Isolation::Auto),
            "isolate" => Some(Isolation::Isolate),
            _ => None,
        }
    }
}

// TODO: explain
pub(crate) fn convert_group(
    node: SvgNode,
    state: &State,
    force: bool,
    cache: &mut Cache,
    parent: &mut Group,
    collect_children: &dyn Fn(&mut Cache, &mut Group),
) -> Option<Group> {
    let (mut group, is_g_or_use) = initial_group(node, state, parent);
    collect_children(cache, &mut group);
    let object_bbox = group.calculate_object_bbox();
    let clip_path = super::clippath::convert_reference(node, state, object_bbox, cache)?;
    let mask = group_mask(node, state, object_bbox, cache)?;
    let filters = group_filters(node, state, object_bbox, cache)?;
    if !group_required(&group, is_g_or_use, force, &clip_path, &mask, &filters) {
        parent.children.append(&mut group.children);
        return None;
    }
    group.clip_path = clip_path;
    group.mask = mask;
    group.filters = filters;
    group.calculate_bounding_boxes();
    Some(group)
}

fn initial_group(node: SvgNode, state: &State, parent: &Group) -> (Group, bool) {
    let opacity = group_opacity(node, state);
    let is_root_svg = node.tag_name() == Some(EId::Svg) && node.parent_element().is_none();
    let transform = if is_root_svg {
        Transform::default()
    } else {
        node.resolve_transform(AId::Transform, state)
    };
    let blend_mode: BlendMode = node.attribute(AId::MixBlendMode).unwrap_or_default();
    let isolation: Isolation = node.attribute(AId::Isolation).unwrap_or_default();
    let isolate = isolation == Isolation::Isolate;
    let is_g_or_use = matches!(node.tag_name(), Some(EId::G) | Some(EId::Use));
    let id = if is_g_or_use && state.parent_markers.is_empty() {
        node.element_id().to_string()
    } else {
        String::new()
    };
    let abs_transform = parent.abs_transform.pre_concat(transform);
    (group_from_properties(id, transform, abs_transform, opacity, blend_mode, isolate), is_g_or_use)
}

fn group_opacity(node: SvgNode, state: &State) -> Opacity {
    if state.parent_clip_path.is_none() {
        node.attribute::<Opacity>(AId::Opacity).unwrap_or(Opacity::ONE)
    } else {
        Opacity::ONE
    }
}

fn group_from_properties(
    id: String,
    transform: Transform,
    abs_transform: Transform,
    opacity: Opacity,
    blend_mode: BlendMode,
    isolate: bool,
) -> Group {
    let dummy = Rect::from_xywh(0.0, 0.0, 0.0, 0.0).unwrap();
    Group {
        id,
        transform,
        abs_transform,
        opacity,
        blend_mode,
        isolate,
        clip_path: None,
        mask: None,
        filters: Vec::new(),
        is_context_element: false,
        bounding_box: dummy,
        abs_bounding_box: dummy,
        stroke_bounding_box: dummy,
        abs_stroke_bounding_box: dummy,
        layer_bounding_box: NonZeroRect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap(),
        abs_layer_bounding_box: NonZeroRect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap(),
        children: Vec::new(),
    }
}

fn group_mask(
    node: SvgNode,
    state: &State,
    bbox: Option<NonZeroRect>,
    cache: &mut Cache,
) -> Option<Option<Arc<Mask>>> {
    let mut mask = None;
    if state.parent_clip_path.is_none() {
        if let Some(link) = node.attribute::<SvgNode>(AId::Mask) {
            mask = super::mask::convert(link, state, bbox, cache);
            if mask.is_none() {
                return None;
            }
        }
    }
    Some(mask)
}

fn group_filters(
    node: SvgNode,
    state: &State,
    bbox: Option<NonZeroRect>,
    cache: &mut Cache,
) -> Option<Vec<Arc<filter::Filter>>> {
    if state.parent_clip_path.is_some() || node.attribute(AId::Filter) == Some("none") {
        return Some(Vec::new());
    }
    super::filter::convert(node, state, bbox, cache).ok()
}

fn group_required(
    group: &Group,
    is_g_or_use: bool,
    force: bool,
    clip_path: &Option<Arc<ClipPath>>,
    mask: &Option<Arc<Mask>>,
    filters: &[Arc<filter::Filter>],
) -> bool {
    group.opacity.get().approx_ne_ulps(&1.0, 4)
        || clip_path.is_some()
        || mask.is_some()
        || !filters.is_empty()
        || !group.transform.is_identity()
        || group.blend_mode != BlendMode::Normal
        || group.isolate
        || is_g_or_use
        || force
}

struct PreparedPath {
    fill: Option<Fill>,
    stroke: Option<Stroke>,
    visibility: Visibility,
    visible: bool,
    rendering_mode: ShapeRendering,
    raw_paint_order: svgtypes::PaintOrder,
    paint_order: PaintOrder,
    transform: Transform,
}

fn convert_path(
    node: SvgNode,
    tiny_skia_path: Arc<tiny_skia_path::Path>,
    state: &State,
    cache: &mut Cache,
    parent: &mut Group,
) {
    /* WHY: tiny-skiaのPathBuilderは要素が一つ以下のPathを生成しない。 */
    debug_assert!(tiny_skia_path.len() >= 2);
    let prepared = prepare_path(node, &tiny_skia_path, state, cache, parent);
    let marker = path_markers(node, &tiny_skia_path, state, cache, parent, &prepared);
    let raw_paint_order = prepared.raw_paint_order;
    let path = match make_path(node, tiny_skia_path, state, prepared) {
        Some(v) => v,
        None => return,
    };
    append_ordered_path(path, raw_paint_order, marker, parent);
}

fn prepare_path(
    node: SvgNode,
    path: &tiny_skia_path::Path,
    state: &State,
    cache: &mut Cache,
    parent: &Group,
) -> PreparedPath {
    let has_bbox = path.bounds().width() > 0.0 && path.bounds().height() > 0.0;
    let mut fill = super::style::resolve_fill(node, has_bbox, state, cache);
    let mut stroke = super::style::resolve_stroke(node, has_bbox, state, cache);
    let visibility: Visibility = node.find_attribute(AId::Visibility).unwrap_or_default();
    let mut visible = visibility == Visibility::Visible;
    let rendering_mode: ShapeRendering = node
        .find_attribute(AId::ShapeRendering)
        .unwrap_or(state.opt.shape_rendering);
    let raw_paint_order: svgtypes::PaintOrder = node.find_attribute(AId::PaintOrder).unwrap_or_default();
    let paint_order = svg_paint_order_to_usvg(raw_paint_order);
    let transform = parent.abs_transform;
    if fill.is_none() && stroke.is_none() {
        visible = false;
    }
    if let Some(fill) = fill.as_mut() {
        process_path_context(&mut fill.paint, &mut fill.context_element, transform, path, cache);
    }
    if let Some(stroke) = stroke.as_mut() {
        process_path_context(&mut stroke.paint, &mut stroke.context_element, transform, path, cache);
    }
    PreparedPath { fill, stroke, visibility, visible, rendering_mode, raw_paint_order, paint_order, transform }
}

fn process_path_context(
    paint: &mut Paint,
    context: &mut Option<ContextElement>,
    transform: Transform,
    path: &tiny_skia_path::Path,
    cache: &mut Cache,
) {
    if let Some(ContextElement::PathNode(context_transform, context_bbox)) = *context {
        process_paint(
            paint,
            true,
            context_transform,
            context_bbox.map(|r| r.to_rect()),
            transform,
            path.bounds(),
            cache,
        );
        *context = None;
    }
}

fn path_markers(
    node: SvgNode,
    path: &tiny_skia_path::Path,
    state: &State,
    cache: &mut Cache,
    parent: &Group,
    prepared: &PreparedPath,
) -> Option<Group> {
    if !marker::is_valid(node) || prepared.visibility != Visibility::Visible {
        return None;
    }
    let mut marker_group = Group { abs_transform: parent.abs_transform, ..Group::empty() };
    let mut marker_state = state.clone();
    let bbox = path.compute_tight_bounds().and_then(|r| r.to_non_zero_rect());
    let fill = prepared.fill.clone().map(|mut f| {
        f.context_element = Some(ContextElement::PathNode(prepared.transform, bbox));
        f
    });
    let stroke = prepared.stroke.clone().map(|mut s| {
        s.context_element = Some(ContextElement::PathNode(prepared.transform, bbox));
        s
    });
    marker_state.context_element = Some((fill, stroke));
    marker::convert(node, path, &marker_state, cache, &mut marker_group);
    marker_group.calculate_bounding_boxes();
    Some(marker_group)
}

fn make_path(
    node: SvgNode,
    tiny_skia_path: Arc<tiny_skia_path::Path>,
    state: &State,
    prepared: PreparedPath,
) -> Option<Path> {
    let id = if state.parent_markers.is_empty() {
        node.element_id().to_string()
    } else {
        String::new()
    };
    Path::new(
        id,
        prepared.visible,
        prepared.fill,
        prepared.stroke,
        prepared.paint_order,
        prepared.rendering_mode,
        tiny_skia_path,
        prepared.transform,
    )
}

fn append_ordered_path(path: Path, raw_paint_order: svgtypes::PaintOrder, marker: Option<Group>, parent: &mut Group) {
    match (raw_paint_order.order, marker) {
        ([PaintOrderKind::Markers, _, _], Some(markers_node)) => {
            parent.children.push(Node::Group(Box::new(markers_node)));
            parent.children.push(Node::Path(Box::new(path.clone())));
        }
        ([first, PaintOrderKind::Markers, _], Some(markers_node)) => {
            // WHY: svgtypesの解析結果ではfill、stroke、markersが1回ずつ現れる。
            let stroke_first = matches!(first, PaintOrderKind::Stroke);
            append_single_paint_path(stroke_first, &path, parent);
            parent.children.push(Node::Group(Box::new(markers_node)));
            append_single_paint_path(!stroke_first, &path, parent);
        }
        ([_, _, PaintOrderKind::Markers], Some(markers_node)) => {
            parent.children.push(Node::Path(Box::new(path.clone())));
            parent.children.push(Node::Group(Box::new(markers_node)));
        }
        _ => parent.children.push(Node::Path(Box::new(path.clone()))),
    }
}

fn append_single_paint_path(stroke_only: bool, path: &Path, parent: &mut Group) {
    let paint_exists = if stroke_only { path.stroke.is_some() } else { path.fill.is_some() };
    if !paint_exists {
        return;
    }
    let mut single_paint_path = path.clone();
    if stroke_only {
        single_paint_path.fill = None;
    } else {
        single_paint_path.stroke = None;
    }
    single_paint_path.id = String::new();
    parent.children.push(Node::Path(Box::new(single_paint_path)));
}

pub fn svg_paint_order_to_usvg(order: svgtypes::PaintOrder) -> PaintOrder {
    match (order.order[0], order.order[1]) {
        (svgtypes::PaintOrderKind::Stroke, _) => PaintOrder::StrokeAndFill,
        (svgtypes::PaintOrderKind::Markers, svgtypes::PaintOrderKind::Stroke) => {
            PaintOrder::StrokeAndFill
        }
        _ => PaintOrder::FillAndStroke,
    }
}

impl SvgNode<'_, '_> {
    pub(crate) fn resolve_transform(&self, transform_aid: AId, state: &State) -> Transform {
        let mut transform: Transform = self.attribute(transform_aid).unwrap_or_default();
        let transform_origin = self
            .attribute::<TransformOrigin>(AId::TransformOrigin)
            .or_else(|| default_transform_origin(*self));

        if let Some(transform_origin) = transform_origin {
            transform = apply_transform_origin(*self, state, transform, transform_origin);
        }

        transform
    }
}

fn default_transform_origin(node: SvgNode) -> Option<TransformOrigin> {
    if node.tag_name() == Some(EId::Svg) && node.parent_element().is_none() {
        Some(TransformOrigin::new(
            Length::new(50.0, Unit::Percent),
            Length::new(50.0, Unit::Percent),
            Length::zero(),
        ))
    } else {
        None
    }
}

fn apply_transform_origin(node: SvgNode, state: &State, transform: Transform, origin: TransformOrigin) -> Transform {
    let dx = convert_length(origin.x_offset, node, AId::Width, Units::UserSpaceOnUse, state);
    let dy = convert_length(origin.y_offset, node, AId::Height, Units::UserSpaceOnUse, state);
    Transform::default()
        .pre_translate(dx, dy)
        .pre_concat(transform)
        .pre_translate(-dx, -dy)
}

#[cfg(test)]
crate::svg_engine::owned_svg_model_tests!("../../../../src/svg_engine_converter_id_collision_tests.rs");
