use serde_json::json;

pub(super) fn assert_c4_observations(envelope: &serde_json::Value) -> Result<(), String> {
    let observations = envelope
        .get("c4GeometryObservations")
        .and_then(serde_json::Value::as_array)
        .ok_or("C4 geometry observations are missing")?;
    if observations.is_empty() {
        return Err("C4 geometry observations are empty".into());
    }
    let mut observed_text = false;
    let mut observed_group = false;
    for row in observations {
        let local_name = c4_observation_type(row)?;
        observed_text |= local_name == "text";
        observed_group |= local_name == "g";
        assert_valid_c4_box(row)?;
    }
    if !observed_text || !observed_group {
        return Err("C4 text and group geometry observations are incomplete".into());
    }
    Ok(())
}

fn c4_observation_type(row: &serde_json::Value) -> Result<&str, String> {
    let local_name = row["node"]["localName"]
        .as_str()
        .ok_or("C4 geometry observation node type is missing")?;
    if !matches!(
        local_name,
        "text" | "g" | "rect" | "circle" | "svg" | "line"
    ) {
        return Err("C4 geometry observation node type is unsupported".into());
    }
    Ok(local_name)
}

fn assert_valid_c4_box(row: &serde_json::Value) -> Result<(), String> {
    let fields = row["box"]
        .as_object()
        .ok_or("C4 geometry observation box is missing")?;
    let coordinate = |name: &str| {
        fields
            .get(name)
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite())
            .ok_or_else(|| format!("C4 geometry observation {name} is not finite numeric data"))
    };
    coordinate("x")?;
    coordinate("y")?;
    if coordinate("width")? < 0.0 || coordinate("height")? < 0.0 {
        return Err("C4 geometry observation has a negative size".into());
    }
    Ok(())
}

fn valid_c4_box() -> serde_json::Value {
    json!({"x": 0.0, "y": 1.0, "width": 2.0, "height": 3.0})
}

fn c4_observation(local_name: &str, box_data: serde_json::Value) -> serde_json::Value {
    json!({"node": {"localName": local_name}, "box": box_data})
}

fn c4_rows(local_names: &[&str]) -> serde_json::Value {
    json!(
        local_names
            .iter()
            .map(|name| c4_observation(name, valid_c4_box()))
            .collect::<Vec<_>>()
    )
}

fn c4_rows_with_text_box(box_data: serde_json::Value) -> serde_json::Value {
    json!([
        c4_observation("text", box_data),
        c4_observation("g", valid_c4_box()),
    ])
}

fn c4_type_cases() -> Vec<(&'static str, serde_json::Value, bool)> {
    vec![
        ("empty observations", c4_rows(&[]), false),
        ("missing text", c4_rows(&["g"]), false),
        ("missing group", c4_rows(&["text"]), false),
        (
            "unsupported node type",
            c4_rows(&["text", "g", "path"]),
            false,
        ),
        (
            "valid supported C4 node boxes",
            c4_rows(&["text", "g", "svg", "line", "rect", "circle"]),
            true,
        ),
    ]
}

fn c4_box_cases() -> Vec<(&'static str, serde_json::Value, bool)> {
    vec![
        (
            "missing coordinate",
            json!({"x": 0, "y": 1, "width": 2}),
            false,
        ),
        (
            "non numeric coordinate",
            json!({"x": "0", "y": 1, "width": 2, "height": 3}),
            false,
        ),
        (
            "negative size",
            json!({"x": 0, "y": 1, "width": -2, "height": 3}),
            false,
        ),
    ]
    .into_iter()
    .map(|(name, box_data, valid)| (name, c4_rows_with_text_box(box_data), valid))
    .collect()
}

#[test]
fn c4_geometry_observations_require_text_group_and_valid_boxes() {
    for (name, observations, succeeds) in c4_type_cases().into_iter().chain(c4_box_cases()) {
        let result = assert_c4_observations(&json!({"c4GeometryObservations": observations}));
        assert_eq!(result.is_ok(), succeeds, "{name}");
    }
}
