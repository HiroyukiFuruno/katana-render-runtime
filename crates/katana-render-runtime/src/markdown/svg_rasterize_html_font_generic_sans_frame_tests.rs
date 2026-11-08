use super::{
    frame_support::invalid_frames, initial_frame_sha, interaction_session, record_raster_phase,
    validate_raster_frame,
};

const SHA256_HEX_LEN: usize = 64;

#[test]
fn closed_interaction_cannot_record_raster_phase() -> Result<(), String> {
    let mut session = interaction_session()?;
    assert!(session.latest_frame().is_some());
    session.close().map_err(|error| error.to_string())?;
    let observer = super::html::raster_observer::Scope::enter();
    assert_eq!(
        record_raster_phase(&session, &observer, "closed"),
        Err("fixture produced no frame".to_owned())
    );
    Ok(())
}

#[test]
fn real_interaction_frame_rejects_inconsistent_rgba_dimensions() -> Result<(), String> {
    let mut session = interaction_session()?;
    let frame = session.latest_frame().ok_or("real frame missing")?.clone();
    session.close().map_err(|error| error.to_string())?;
    validate_raster_frame(&frame)?;
    let digest = initial_frame_sha(&frame)?;
    assert_eq!(digest.len(), SHA256_HEX_LEN);
    let [width, height, pixels, overflow] = invalid_frames(&frame);
    for invalid in [&width, &height, &pixels] {
        assert_eq!(
            validate_raster_frame(invalid).err().as_deref(),
            Some("fixture frame is not valid RGBA8888")
        );
        assert_eq!(
            initial_frame_sha(invalid).err().as_deref(),
            Some("initial fixture frame has invalid RGBA dimensions")
        );
    }
    assert_eq!(
        validate_raster_frame(&overflow).err().as_deref(),
        Some("frame dimensions overflow")
    );
    assert_eq!(
        initial_frame_sha(&overflow).err().as_deref(),
        Some("initial fixture frame dimensions overflow")
    );
    Ok(())
}
