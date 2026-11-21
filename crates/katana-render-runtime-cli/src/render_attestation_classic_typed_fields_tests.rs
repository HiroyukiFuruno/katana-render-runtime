use super::{TARGET, observe, programs, rejected, request, request_json};

#[test]
fn unknown_phase_extra_fields_and_debug_strings_are_rejected() -> anyhow::Result<()> {
    rejected(
        observe(|| {
            tracing::event!(target: TARGET, tracing::Level::TRACE, phase = "unknown");
        })?,
        "unknown classic event phase",
    )?;
    rejected(
        observe(|| {
            tracing::event!(target: TARGET, tracing::Level::TRACE,
            phase = "complete", unexpected = "field");
        })?,
        "invalid classic event field",
    )?;
    rejected(
        observe(|| {
            tracing::event!(target: TARGET, tracing::Level::TRACE, phase = ?"complete");
        })?,
        "invalid classic event field",
    )
}

#[test]
fn duplicate_string_and_unexpected_numeric_fields_are_rejected() -> anyhow::Result<()> {
    let events: [fn(); 2] = [
        || {
            tracing::event!(target: TARGET, tracing::Level::TRACE,
                phase = "complete", phase = "complete");
        },
        || {
            tracing::event!(target: TARGET, tracing::Level::TRACE,
                phase = "complete", unexpected = 1_u64);
        },
    ];
    for emit in events {
        rejected(observe(emit)?, "invalid classic event field")?;
    }
    Ok(())
}

#[test]
fn type_count_and_event_level_are_rejected() -> anyhow::Result<()> {
    let value = request_json(false);
    rejected(
        observe(|| {
            request(&value, 5);
            programs(&value, true);
        })?,
        "type/count differs",
    )?;
    rejected(observe(|| request(&value, 3))?, "script count differs")?;
    rejected(
        observe(|| {
            tracing::event!(target: TARGET, tracing::Level::INFO, phase = "complete");
        })?,
        "invalid classic event field",
    )
}
