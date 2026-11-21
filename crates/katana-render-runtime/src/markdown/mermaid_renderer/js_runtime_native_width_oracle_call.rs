use super::{TEXT_METRIC_ITALIC_ARGUMENT_INDEX, TEXT_METRIC_WEIGHT_ARGUMENT_INDEX};
use serde_json::Value;

pub(super) struct ParsedCall<'a> {
    pub(super) text: &'a str,
    pub(super) family: &'a str,
    pub(super) size: f32,
    pub(super) weight: u16,
    pub(super) italic: bool,
    pub(super) actual: f64,
}

impl<'a> ParsedCall<'a> {
    pub(super) fn from_json(call: &'a Value) -> Result<Self, String> {
        let args = call
            .get("args")
            .and_then(Value::as_array)
            .ok_or("arguments missing")?;
        Ok(Self {
            text: text_argument(args, 0)?,
            family: text_argument(args, 1)?,
            size: number_argument(args, 2)? as f32,
            weight: number_argument(args, TEXT_METRIC_WEIGHT_ARGUMENT_INDEX)? as u16,
            italic: args
                .get(TEXT_METRIC_ITALIC_ARGUMENT_INDEX)
                .and_then(Value::as_bool)
                .ok_or("italic missing")?,
            actual: call
                .get("width")
                .and_then(Value::as_f64)
                .ok_or("width missing")?,
        })
    }
}

fn text_argument(args: &[Value], index: usize) -> Result<&str, String> {
    args.get(index)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("text argument {index} missing"))
}

fn number_argument(args: &[Value], index: usize) -> Result<f64, String> {
    args.get(index)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("numeric argument {index} missing"))
}
