const DEFAULT_ROTATION_DEGREES_PER_TURN: f32 = 360.0;

pub(super) fn css_rotation_degrees(value: &str) -> Option<f32> {
    let value = value
        .strip_prefix("rotate(")?
        .strip_suffix(')')?
        .trim()
        .to_ascii_lowercase();
    let rotation = if let Some(degrees) = value.strip_suffix("deg") {
        degrees.trim().parse::<f32>().ok()?
    } else if let Some(turns) = value.strip_suffix("turn") {
        turns.trim().parse::<f32>().ok()? * DEFAULT_ROTATION_DEGREES_PER_TURN
    } else {
        let radians = value.strip_suffix("rad")?;
        radians.trim().parse::<f32>().ok()?.to_degrees()
    };
    rotation.is_finite().then_some(rotation)
}
