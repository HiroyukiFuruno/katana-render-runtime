use super::{BINARY_FIXTURE_SVG, Context, binary_context_input, usvg};
use std::sync::{
    Arc, PoisonError,
    atomic::{AtomicBool, Ordering},
};

#[test]
fn poisoned_selected_face_attempt_rejects_svg_admission_and_clears_state() -> Result<(), String> {
    let (database, proof, _) = binary_context_input(1)?;
    let context =
        Context::new(Arc::clone(&database), proof, "AB".into()).ok_or("binary context missing")?;
    let attempt = context.begin().ok_or("attempt missing")?;
    poison_attempt_mutex(&context);
    let valid = Arc::new(AtomicBool::new(true));
    let selected = Arc::new(AtomicBool::new(false));
    let mut options = usvg::Options {
        fontdb: Arc::clone(&database),
        ..usvg::Options::default()
    };
    context.install(&mut options, Arc::clone(&valid), Arc::clone(&selected));
    let parsed = usvg::Tree::from_str(BINARY_FIXTURE_SVG, &options);
    assert!(parsed.is_ok());
    assert!(selected.load(Ordering::Relaxed));
    assert!(!valid.load(Ordering::Relaxed));
    assert!(!context.finish(&database));
    drop(attempt);
    let state = context.state.lock().unwrap_or_else(PoisonError::into_inner);
    assert!(state.is_none());
    Ok(())
}

fn poison_attempt_mutex(context: &Context) {
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _state = context.state.lock().unwrap_or_else(PoisonError::into_inner);
        std::panic::panic_any("selected face attempt interrupted while locked");
    }));
    assert!(unwind.is_err());
    assert!(context.state.is_poisoned());
}
