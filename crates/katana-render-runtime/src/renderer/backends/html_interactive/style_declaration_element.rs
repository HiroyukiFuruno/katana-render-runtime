use super::CssStyle;
impl CssStyle {
    pub(super) fn apply_element_declarations(
        &mut self,
        source: &str,
        inherited_font_size: f32,
        inherited_vertical_align: super::CssVerticalAlign,
        tag: Option<&str>,
    ) {
        let declarations = super::super::style_declaration_cache::get_or_parse(source);
        self.apply_font_declarations(&declarations, inherited_font_size);
        if let Some(tag) = tag {
            self.refresh_tag_margins(tag);
        }
        self.apply_non_font_declarations(
            &declarations,
            inherited_vertical_align,
            self.text_spacing_trim,
        );
    }

    fn apply_font_declarations(
        &mut self,
        declarations: &[crate::renderer::backends::html_css_rule::CssDeclaration],
        inherited_font_size: f32,
    ) {
        for declaration in declarations.iter().filter(|declaration| {
            declaration.name.eq_ignore_ascii_case("font-size")
                || declaration.name.eq_ignore_ascii_case("font")
        }) {
            self.font_size = inherited_font_size;
            self.apply(&declaration.name, &declaration.value);
        }
    }

    fn apply_non_font_declarations(
        &mut self,
        declarations: &[crate::renderer::backends::html_css_rule::CssDeclaration],
        inherited_vertical_align: super::CssVerticalAlign,
        inherited_text_spacing_trim: super::CssTextSpacingTrim,
    ) {
        let mut line_heights = Vec::new();
        for declaration in declarations {
            self.apply_non_font_declaration(
                declaration,
                inherited_vertical_align,
                inherited_text_spacing_trim,
                &mut line_heights,
            );
        }
        self.resolve_inherited_line_height();
        for line_height in line_heights {
            self.apply_line_height(line_height);
        }
        if !declarations
            .iter()
            .any(|declaration| declaration.name.eq_ignore_ascii_case("column-gap"))
        {
            self.column_gap = self.font_size;
        }
    }

    fn apply_non_font_declaration<'a>(
        &mut self,
        declaration: &'a crate::renderer::backends::html_css_rule::CssDeclaration,
        inherited_vertical_align: super::CssVerticalAlign,
        inherited_text_spacing_trim: super::CssTextSpacingTrim,
        line_heights: &mut Vec<&'a str>,
    ) {
        if declaration.name.eq_ignore_ascii_case("line-height") {
            line_heights.push(declaration.value.as_str());
            return;
        }
        if declaration.name.eq_ignore_ascii_case("vertical-align") {
            self.apply_vertical_align(&declaration.value, inherited_vertical_align);
            return;
        }
        if declaration.name.eq_ignore_ascii_case("font-size")
            || declaration.name.eq_ignore_ascii_case("font")
        {
            return;
        }
        if self.apply_inherited_text_spacing(declaration, inherited_text_spacing_trim) {
            return;
        }
        self.apply(&declaration.name, &declaration.value);
    }

    fn apply_inherited_text_spacing(
        &mut self,
        declaration: &crate::renderer::backends::html_css_rule::CssDeclaration,
        inherited: super::CssTextSpacingTrim,
    ) -> bool {
        if declaration.name.eq_ignore_ascii_case("text-spacing-trim")
            && matches!(
                declaration.value.trim().to_ascii_lowercase().as_str(),
                "inherit" | "unset"
            )
        {
            self.text_spacing_trim = inherited;
            return true;
        }
        false
    }
}
