use super::super::super::proof::same_face;
use resvg::usvg::fontdb::{Database, FaceInfo, Family, ID};
use std::sync::{Arc, Weak};

#[derive(Clone)]
pub(super) struct Alias {
    pub(super) origin: ID,
    pub(super) face: FaceInfo,
    pub(super) database: Weak<Database>,
}

impl Alias {
    pub(super) fn in_database(&self, database: &Arc<Database>) -> bool {
        Weak::ptr_eq(&self.database, &Arc::downgrade(database))
    }

    pub(super) fn matches(&self, origin: &FaceInfo, database: &Arc<Database>) -> bool {
        let mut expected = origin.clone();
        expected.id = self.face.id;
        self.in_database(database)
            && same_face(&expected, &self.face)
            && database
                .face(self.face.id)
                .is_some_and(|face| same_face(face, &self.face))
    }
}

pub(super) fn database_current(
    original: &Arc<Database>,
    database: &Arc<Database>,
    aliases: &[Alias],
) -> bool {
    if database.len() < original.len() || !same_generic(original, database) {
        return false;
    }
    let prefix_current = Arc::ptr_eq(original, database)
        || original
            .faces()
            .zip(database.faces())
            .all(|(left, right)| same_face(left, right));
    let registered_count = aliases
        .iter()
        .filter(|alias| alias.in_database(database))
        .count();
    prefix_current
        && database.len() == original.len() + registered_count
        && aliases
            .iter()
            .filter(|alias| alias.in_database(database))
            .all(|alias| {
                database
                    .face(alias.face.id)
                    .is_some_and(|face| same_face(face, &alias.face))
            })
}

fn same_generic(left: &Database, right: &Database) -> bool {
    [
        Family::Serif,
        Family::SansSerif,
        Family::Cursive,
        Family::Fantasy,
        Family::Monospace,
    ]
    .iter()
    .all(|family| left.family_name(family) == right.family_name(family))
}
