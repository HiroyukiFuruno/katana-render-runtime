use super::{DatabaseKey, FontSourceGeneration};
use super::{HtmlFontRequest, HtmlTextMeasurementToken, Proof};
use resvg::usvg::fontdb::Database;
use std::{mem::size_of, rc::Rc, sync::Arc};

const MAX_TOKENS: usize = 8;
const MAX_TOKEN_BYTES: usize = 256 * 1024;

pub(super) struct StoredToken {
    request: DatabaseKey,
    token: HtmlTextMeasurementToken,
    bytes: usize,
}

pub(super) struct TokenStore {
    entries: Vec<StoredToken>,
}

impl TokenStore {
    pub(super) fn build_token(
        request: &HtmlFontRequest,
        database: Arc<Database>,
        generations: Vec<(resvg::usvg::fontdb::ID, FontSourceGeneration)>,
    ) -> Option<(HtmlTextMeasurementToken, usize)> {
        let bytes = token_bytes(request, generations.capacity());
        (bytes <= MAX_TOKEN_BYTES).then(|| (new_token(database, generations), bytes))
    }
}

#[cfg(test)]
#[path = "svg_rasterize_text_measurement_token_store_tests.rs"]
mod tests;

fn new_token(
    database: Arc<Database>,
    generations: Vec<(resvg::usvg::fontdb::ID, FontSourceGeneration)>,
) -> HtmlTextMeasurementToken {
    HtmlTextMeasurementToken {
        proof: Rc::new(Proof {
            database: Arc::downgrade(&database),
            generations,
            _not_send_sync: std::marker::PhantomData,
        }),
    }
}

fn same_database_and_generations(
    token: &HtmlTextMeasurementToken,
    database: &Arc<Database>,
    generations: &[(resvg::usvg::fontdb::ID, FontSourceGeneration)],
) -> bool {
    let Some(cached) = token.proof.database.upgrade() else {
        return false;
    };
    Arc::ptr_eq(&cached, database) && token.proof.generations == generations
}

fn token_bytes(request: &HtmlFontRequest, generation_capacity: usize) -> usize {
    size_of::<Proof>()
        + size_of::<StoredToken>()
        + size_of::<HtmlTextMeasurementToken>()
        + size_of::<Rc<Proof>>()
        + generation_capacity * size_of::<(resvg::usvg::fontdb::ID, FontSourceGeneration)>()
        + request.families.iter().map(String::capacity).sum::<usize>()
        + request.families.capacity() * size_of::<String>()
}

impl TokenStore {
    pub(super) const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub(super) fn get_key(
        &self,
        request: &DatabaseKey,
        database: &Arc<Database>,
        generations: &[(resvg::usvg::fontdb::ID, super::FontSourceGeneration)],
    ) -> Option<HtmlTextMeasurementToken> {
        self.entries
            .iter()
            .find(|entry| {
                entry.request == *request
                    && entry.token.is_usable()
                    && same_database_and_generations(&entry.token, database, generations)
            })
            .map(|entry| entry.token.clone())
    }

    pub(super) fn insert_key(
        &mut self,
        request: DatabaseKey,
        token: HtmlTextMeasurementToken,
        bytes: usize,
    ) {
        self.entries.push(StoredToken {
            request,
            token,
            bytes,
        });
        while self.entries.len() > MAX_TOKENS {
            self.entries.remove(0);
        }
        self.entries.shrink_to_fit();
        while self.capacity_bytes() > MAX_TOKEN_BYTES && !self.entries.is_empty() {
            self.entries.remove(0);
            self.entries.shrink_to_fit();
        }
    }

    fn capacity_bytes(&self) -> usize {
        self.entries.capacity() * size_of::<StoredToken>()
            + self.entries.iter().map(|entry| entry.bytes).sum::<usize>()
    }
}

#[cfg(test)]
impl TokenStore {
    pub(super) fn get(
        &self,
        request: &HtmlFontRequest,
        database: &Arc<Database>,
        generations: &[(resvg::usvg::fontdb::ID, FontSourceGeneration)],
    ) -> Option<HtmlTextMeasurementToken> {
        self.get_key(&DatabaseKey::eager(request.clone()), database, generations)
    }

    pub(super) fn insert(
        &mut self,
        request: HtmlFontRequest,
        token: HtmlTextMeasurementToken,
        bytes: usize,
    ) {
        self.insert_key(DatabaseKey::eager(request), token, bytes);
    }
}
