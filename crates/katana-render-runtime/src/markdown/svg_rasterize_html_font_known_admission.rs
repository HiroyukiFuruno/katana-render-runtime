use super::super::super::text_shaping::{
    known_family_names, known_matching, known_memo_usable, known_observe_all, known_supports,
};
use super::proof::CreationProof;
use resvg::usvg::fontdb::{self, Database, ID};
use std::sync::Arc;

pub(super) struct Admission<'a> {
    database: &'a Arc<Database>,
    proof: &'a CreationProof,
    selected: ID,
}

impl Admission<'_> {
    pub(super) fn selected(&self) -> ID {
        self.selected
    }

    pub(super) fn matches(&self, proof: &CreationProof, database: &Database, id: ID) -> bool {
        std::ptr::eq(self.proof, proof)
            && std::ptr::eq(self.database.as_ref(), database)
            && self.selected == id
    }
}

pub(super) fn admit(
    database: &Arc<Database>,
    proof: &CreationProof,
    family: &str,
    text: &str,
    weight: u16,
    italic: bool,
) -> Option<ID> {
    admit_face(database, proof, family, text, weight, italic).map(|receipt| receipt.selected())
}

pub(super) fn admit_face<'a>(
    database: &'a Arc<Database>,
    proof: &'a CreationProof,
    family: &str,
    text: &str,
    weight: u16,
    italic: bool,
) -> Option<Admission<'a>> {
    if !text.is_ascii() || !known_memo_usable() || !proof.current(database) {
        return None;
    }
    let names = known_family_names(family);
    let first = names.first()?;
    admit_first_family(database, first, weight, italic)?;
    let selected = known_matching(database, family, weight, italic)?;
    if known_matching(database, first, weight, italic)? != selected
        || !exact_prefix_face(database, proof, selected, weight, italic)
    {
        return None;
    }
    finish_admission(database, proof, selected, text)?;
    Some(Admission {
        database,
        proof,
        selected,
    })
}

fn admit_first_family(database: &Database, first: &str, weight: u16, italic: bool) -> Option<()> {
    if first.eq_ignore_ascii_case("sans-serif") {
        let resolved = database.family_name(&fontdb::Family::SansSerif);
        /* WHY: HTMLの先頭優先familyだけなら、後続discoveryでgenericの選択が上位へ変わらない。 */
        if resolved != super::sources::configured_html_sans_serif_family()
            || known_matching(database, first, weight, italic)?
                != known_matching(database, resolved, weight, italic)?
        {
            return None;
        }
    } else if super::sources::is_generic_font_family(&first.to_ascii_lowercase()) {
        return None;
    }
    Some(())
}

fn finish_admission(
    database: &Arc<Database>,
    proof: &CreationProof,
    selected: ID,
    text: &str,
) -> Option<ID> {
    if !known_observe_all(database)
        || !text
            .chars()
            .all(|character| known_supports(database, selected, character))
    {
        return None;
    }
    (known_memo_usable() && proof.current(database)).then_some(selected)
}

fn exact_prefix_face(
    database: &Database,
    proof: &CreationProof,
    selected: ID,
    weight: u16,
    italic: bool,
) -> bool {
    database.face(selected).is_some_and(|face| {
        proof.contains(selected)
            && face.weight == fontdb::Weight(weight)
            && face.stretch == fontdb::Stretch::Normal
            && face.style
                == if italic {
                    fontdb::Style::Italic
                } else {
                    fontdb::Style::Normal
                }
    })
}

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_font_admission_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_font_generic_sans_tests.rs"]
mod generic_sans_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_font_generic_sans_family_profile_tests.rs"]
mod generic_sans_family_profile_tests;
