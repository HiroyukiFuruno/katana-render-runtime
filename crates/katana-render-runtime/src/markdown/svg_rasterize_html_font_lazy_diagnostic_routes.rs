use super::super::proof::CreationProof;
use super::super::{HtmlFontRequest, discovery::KnownCacheValue, sources as font_sources};
use super::{MODE, Mode, SourceRoute, active, error, sources, stage, update};
use resvg::usvg::fontdb::{Database, ID};
use std::{cell::Cell, sync::Arc, time::Instant};

const SELECTED_RECORDS: usize = 2;
const SHARED_SELECTED_RECORDS: usize = 3;

pub(in super::super) fn file_fixture_database(
    request: &HtmlFontRequest,
) -> Option<KnownCacheValue> {
    /* WHY: 保存形式の対照は実Fileから構築し、製品cacheのSharedFileをFileと偽って計測しない。 */
    let (database, proof) = font_sources::build_known_html_font_db(request);
    validate_file_fixture_database(Arc::new(database), Arc::new(proof))
}

pub(in super::super) fn validate_file_fixture_database(
    database: Arc<Database>,
    proof: Arc<CreationProof>,
) -> Option<KnownCacheValue> {
    proof
        .current(&database)
        .then_some(KnownCacheValue { database, proof })
}

pub(in super::super) fn select_database(
    db: &Arc<Database>,
    proof: Arc<CreationProof>,
) -> Option<(Arc<Database>, Arc<CreationProof>)> {
    let Some(route) = super::ROUTE.with(Cell::get) else {
        return proof.transient(db);
    };
    audit_database(db, &proof, "CreationProof invalid before route setup")?;
    let started = matches!(MODE.with(Cell::get), Mode::Measure).then(Instant::now);
    let selected = select_route(db, proof, route)?;
    if let Some(started) = started {
        stage(route_stage(route), started.elapsed().as_nanos());
    }
    record_database_selection(selected)
}

pub(super) fn record_database_selection(
    selected: (Arc<Database>, Arc<CreationProof>),
) -> Option<(Arc<Database>, Arc<CreationProof>)> {
    audit_database(
        &selected.0,
        &selected.1,
        "CreationProof invalid after route setup",
    )
    .map(|()| {
        update(1, |capture| {
            capture.database_owners.push(Arc::downgrade(&selected.0));
        });
        selected
    })
}
fn audit_database(db: &Database, proof: &CreationProof, message: &'static str) -> Option<()> {
    if matches!(MODE.with(Cell::get), Mode::Audit) && !proof.current(db) {
        error(message);
        return None;
    }
    Some(())
}
fn select_route(
    db: &Arc<Database>,
    proof: Arc<CreationProof>,
    route: SourceRoute,
) -> Option<(Arc<Database>, Arc<CreationProof>)> {
    match route {
        SourceRoute::File => Some((Arc::clone(db), proof)),
        SourceRoute::SharedFile => proof.transient(db),
    }
}
fn route_stage(route: SourceRoute) -> &'static str {
    match route {
        SourceRoute::File => "file_cached_db_proof",
        SourceRoute::SharedFile => "shared_transient_proof",
    }
}
pub(in super::super) fn record_selected_face(db: &Arc<Database>, id: Option<ID>) {
    if !active() {
        return;
    }
    let Some(face) = id.and_then(|id| db.face(id)) else {
        return;
    };
    let Some((record, weak)) = sources::source_record(face, MODE.with(Cell::get)) else {
        error("selected font source could not be audited");
        return;
    };
    let cost = if weak.is_some() {
        SHARED_SELECTED_RECORDS
    } else {
        SELECTED_RECORDS
    };
    update(cost, |capture| {
        capture.faces.push(record);
        capture.selected_database_owners.push(Arc::downgrade(db));
        if let Some(weak) = weak {
            capture.shared_byte_owners.push(weak);
        }
    });
}
