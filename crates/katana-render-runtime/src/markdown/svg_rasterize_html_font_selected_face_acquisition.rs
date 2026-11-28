use super::super::super::admission::Admission;
use super::{AdmittedAttempt, Context, State, reused_alias_id};
use resvg::usvg::{
    self,
    fontdb::{Database, ID, Source},
};
use std::sync::{Arc, MutexGuard};

impl Context {
    pub(super) fn render_admitted_face<'a>(
        &'a self,
        font: &usvg::Font,
        database: &mut Arc<Database>,
        origin: Option<ID>,
        state: MutexGuard<'a, Option<State>>,
    ) -> (Option<ID>, bool) {
        /* WHY: 外部 selector の呼出後に lock し、admission と取得の間に attempt を交替させない。 */
        let Some(admission) =
            super::super::selector_admitted(font, &self.original, &self.proof, &self.text, origin)
        else {
            return (origin.filter(|id| database.face(*id).is_some()), false);
        };
        let origin = admission.selected();
        #[cfg(test)]
        super::super::super::raster_observer::local_font_after_admission();
        let actual = self.render_face(
            database,
            origin,
            Some(AdmittedAttempt {
                context: self,
                state,
                admission,
            }),
        );
        (actual, actual.is_some())
    }

    pub(super) fn render_selected_face(
        &self,
        database: &mut Arc<Database>,
        origin: ID,
        state: &mut State,
        admission: Option<&Admission<'_>>,
    ) -> Option<ID> {
        let alias = state
            .aliases
            .iter()
            .find(|alias| alias.origin == origin && alias.in_database(database));
        let face = self.proof.selected_shared_face(
            &self.original,
            origin,
            &mut state.sources,
            alias.map(|alias| &alias.face.source),
            admission,
        )?;
        if matches!(face.source, Source::Binary(_)) {
            return Some(origin);
        }
        if let Some(alias) = alias {
            return reused_alias_id(alias, &face, database);
        }
        state.append(&self.original, &self.proof, database, origin, face)
    }
}
