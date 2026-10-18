use super::super::super::{proof::CreationProof, shared_raster::shared_sources::SharedSources};
#[cfg(test)]
use super::record_alias;
use super::{MAX_REGISTERED_FACES, seal};
use resvg::usvg::fontdb::{Database, FaceInfo, ID};
use seal::Alias;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct State {
    pub(super) sources: SharedSources,
    pub(super) aliases: Vec<Alias>,
}

impl State {
    fn register_alias(
        &mut self,
        database: &Arc<Database>,
        copied: Vec<Alias>,
        origin: ID,
        id: ID,
    ) -> Option<()> {
        /* WHY: COW後も同じ DB 由来の登録だけを新 namespace に接続する。 */
        self.aliases
            .retain(|alias| alias.database.upgrade().is_some());
        for mut alias in copied {
            alias.database = Arc::downgrade(database);
            self.aliases.push(alias);
        }
        self.aliases.push(Alias {
            origin,
            face: database.face(id)?.clone(),
            database: Arc::downgrade(database),
        });
        Some(())
    }
}

impl State {
    fn admit_append(&self, database: &Database, copied_count: usize) -> Option<()> {
        if self
            .aliases
            .len()
            .checked_add(copied_count)?
            .checked_add(1)?
            > MAX_REGISTERED_FACES
            || database.len() >= MAX_REGISTERED_FACES
        {
            return None;
        }
        Some(())
    }

    pub(super) fn append(
        &mut self,
        original: &Arc<Database>,
        proof: &CreationProof,
        database: &mut Arc<Database>,
        origin: ID,
        mut face: FaceInfo,
    ) -> Option<ID> {
        let copied: Vec<_> = self
            .aliases
            .iter()
            .filter(|alias| alias.in_database(database))
            .cloned()
            .collect();
        self.admit_append(database, copied.len())?;
        face.id = ID::dummy();
        let id = Arc::make_mut(database).push_face_info(face);
        self.register_alias(database, copied, origin, id)?;
        let current =
            proof.current(original) && seal::database_current(original, database, &self.aliases);
        #[cfg(test)]
        if current {
            record_alias("appended", origin, id);
        }
        current.then_some(id)
    }
}
