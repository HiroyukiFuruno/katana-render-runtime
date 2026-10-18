use super::{Attempt, Context, State, seal};
use resvg::usvg::fontdb::Database;
use std::sync::Arc;

impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        /* WHY: unwind と失敗時にも backing を callback owner に残さない。 */
        self.context
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
    }
}

impl Context {
    pub(in super::super) fn begin(&self) -> Option<Attempt<'_>> {
        let mut state = self.state.lock().ok()?;
        if state.is_some() || !self.proof.selected_current(&self.original) {
            return None;
        }
        *state = Some(State::default());
        Some(Attempt {
            context: self,
            #[cfg(test)]
            _observation: super::super::super::raster_observer::AttemptScope::enter(),
        })
    }

    pub(in super::super) fn finish(&self, database: &Arc<Database>) -> bool {
        let Ok(locked) = self.state.lock() else {
            return false;
        };
        let Some(state) = locked.as_ref() else {
            return false;
        };
        self.proof.selected_current(&self.original)
            && seal::database_current(&self.original, database, &state.aliases)
            && state.aliases.iter().all(|alias| {
                alias.database.upgrade().is_none_or(|db| {
                    Arc::ptr_eq(database, &db)
                        || seal::database_current(&self.original, &db, &state.aliases)
                })
            })
    }
}
