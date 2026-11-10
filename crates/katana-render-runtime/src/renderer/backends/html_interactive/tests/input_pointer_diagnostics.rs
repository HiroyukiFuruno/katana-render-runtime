use super::support::{TestResult, click_element, start};
use std::{fmt::Debug, sync::Mutex};
use tracing::{
    Event, Level, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

#[derive(Default)]
struct PointerEvent {
    message: String,
    x: Option<f64>,
    y: Option<f64>,
    pressed_target: String,
    hit: String,
}

impl Visit for PointerEvent {
    fn record_f64(&mut self, field: &Field, value: f64) {
        match field.name() {
            "physical_x" => self.x = Some(value),
            "physical_y" => self.y = Some(value),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        match field.name() {
            "message" => self.message = format!("{value:?}"),
            "pressed_target" => self.pressed_target = format!("{value:?}"),
            "target" => self.hit = format!("{value:?}"),
            _ => {}
        }
    }
}

#[derive(Default)]
struct PointerEvents(Mutex<Vec<PointerEvent>>);

impl Subscriber for PointerEvents {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.is_event()
            && *metadata.level() == Level::DEBUG
            && metadata
                .target()
                .ends_with("::html_interactive::input::dispatch")
    }

    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut fields = PointerEvent::default();
        event.record(&mut fields);
        if let Ok(mut captured) = self.0.lock() {
            captured.push(fields);
        }
    }
}

#[test]
fn pointer_diagnostics_record_the_pressed_node_and_same_release_coordinates() -> TestResult {
    let mut session = start("<input id='target' type='checkbox'>")?;
    let node = session
        .runtime
        .node_for_element_id("target")
        .ok_or("target input missing")?;
    let events = std::sync::Arc::new(PointerEvents::default());
    tracing::subscriber::with_default(events.clone(), || click_element(&mut session, "target"))?;

    let captured = events.0.lock().map_err(|error| error.to_string())?;
    let [down, up] = captured.as_slice() else {
        return Err("expected one pointer down and one pointer up event".to_owned());
    };
    assert_eq!(down.message, "HTML pointer down hit-test");
    assert_eq!(up.message, "HTML pointer up hit-test");
    assert!(down.x.is_some_and(f64::is_finite) && down.y.is_some_and(f64::is_finite));
    assert_eq!((up.x, up.y), (down.x, down.y));
    assert_eq!(up.pressed_target, format!("Some({})", node.0));
    assert!(down.hit.starts_with("Some("));
    assert_eq!(up.hit, down.hit);
    Ok(())
}
