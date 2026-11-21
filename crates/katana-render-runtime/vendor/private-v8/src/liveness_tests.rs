use super::{Global, Isolate, Primitive, initialize_v8, scope};
use std::sync::Arc;

#[test]
fn disposal_releases_the_liveness_cell_when_no_global_is_retained() {
    initialize_v8();
    let isolate = Isolate::new(Default::default());
    let liveness = Arc::downgrade(&isolate.clone_global_liveness());

    drop(isolate);

    assert!(liveness.upgrade().is_none());
}

#[test]
fn late_global_drop_releases_the_disposed_isolates_liveness_cell() {
    initialize_v8();
    let mut isolate = Isolate::new(Default::default());
    let liveness = Arc::downgrade(&isolate.clone_global_liveness());
    let global = {
        scope!(let scope, &mut isolate);
        Global::new(scope, Primitive::null(scope))
    };

    drop(isolate);
    assert!(liveness.upgrade().is_some());
    drop(global);

    assert!(liveness.upgrade().is_none());
}
