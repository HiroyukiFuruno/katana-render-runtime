use super::TARGET;
use super::protocol::{CaptureState, EventFields};
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

const SCRIPT_COUNT_FIELD_BIT: u8 = 4;
const SCRIPT_NAME_FIELD_BIT: u8 = 8;
const SCRIPT_CODE_FIELD_BIT: u8 = 16;

impl EventFields {
    fn mark(&mut self, bit: u8) {
        if self.seen & bit != 0 {
            self.error = Some("duplicate classic event field".into());
        }
        self.seen |= bit;
    }
}

impl Visit for EventFields {
    fn record_str(&mut self, field: &Field, value: &str) {
        let bit = match field.name() {
            "phase" => {
                self.phase = Some(value.to_owned());
                1
            }
            "request_json" => {
                self.request = Some(value.to_owned());
                2
            }
            "name" => {
                self.name = Some(value.to_owned());
                SCRIPT_NAME_FIELD_BIT
            }
            "code" => {
                self.code = Some(value.to_owned());
                SCRIPT_CODE_FIELD_BIT
            }
            _ => {
                self.error = Some("unexpected classic string field".into());
                return;
            }
        };
        self.mark(bit);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() != "script_count" {
            self.error = Some("unexpected classic numeric field".into());
            return;
        }
        self.count = Some(value);
        self.mark(SCRIPT_COUNT_FIELD_BIT);
    }

    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {
        self.error = Some("classic event requires typed fields".into());
    }
}

pub(super) struct CaptureSubscriber {
    pub(super) state: Arc<Mutex<CaptureState>>,
}

impl Subscriber for CaptureSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target() == TARGET && metadata.is_event()
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _span: &Id, _values: &Record<'_>) {}
    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}
    fn enter(&self, _span: &Id) {}
    fn exit(&self, _span: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut fields = EventFields::default();
        event.record(&mut fields);
        if event.metadata().level() != &tracing::Level::TRACE {
            fields.error = Some("classic event level differs".into());
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.event(fields);
    }
}
