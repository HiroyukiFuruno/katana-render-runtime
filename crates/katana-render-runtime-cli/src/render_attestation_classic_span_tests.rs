use super::{CaptureState, CaptureSubscriber, TARGET};
use std::sync::{Arc, Mutex};
use tracing::callsite::Callsite;
use tracing::span::{Attributes, Record};
use tracing::{Metadata, Subscriber};

fn span_metadata() -> &'static Metadata<'static> {
    tracing::callsite! {
        name: "classic-span-boundary",
        kind: tracing::metadata::Kind::SPAN,
        target: TARGET,
        level: tracing::Level::TRACE,
        fields:
    }
    .metadata()
}

#[test]
fn span_lifecycle_callbacks_cannot_become_classic_capture_events() -> anyhow::Result<()> {
    let state = Arc::new(Mutex::new(CaptureState::new()));
    let subscriber = CaptureSubscriber {
        state: Arc::clone(&state),
    };
    let metadata = span_metadata();
    assert!(!subscriber.enabled(metadata));
    let values = metadata.fields().value_set(&[]);
    let span = subscriber.new_span(&Attributes::new(metadata, &values));
    subscriber.record(&span, &Record::new(&values));
    subscriber.record_follows_from(&span, &span);
    subscriber.enter(&span);
    subscriber.exit(&span);
    let state = state
        .lock()
        .map_err(|_| anyhow::anyhow!("capture lock poisoned"))?;
    assert!(state.request.is_none());
    assert!(state.scripts.is_empty());
    Ok(())
}
