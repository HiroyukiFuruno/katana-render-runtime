use super::{Metric, count, face, record};

pub(super) fn install_eager_styled_fallback(options: &mut resvg::usvg::Options<'static>) {
    if let Some(styled) = options.font_resolver.select_styled_fallback.take() {
        options.font_resolver.select_styled_fallback = Some(Box::new(
            move |font, character, excluded, database| {
                let actual = styled(font, character, excluded, database);
                count(Metric::EagerFallback);
                super::super::super::lazy_diagnostic::record_actual_selected_face(database, actual);
                record(|| {
                    serde_json::json!({"stage":"fallback", "route":"eager", "character":character.to_string(),
                "codepoint":character as u32, "requestedWeight":font.weight(), "requestedStyle":format!("{:?}",font.style()),
                "requestedStretch":format!("{:?}",font.stretch()), "excludedLocalIds":excluded.iter().map(|id| format!("{id:?}")).collect::<Vec<_>>(),
                "actualFace":face(database, actual)})
                });
                actual
            },
        ));
    }
}
