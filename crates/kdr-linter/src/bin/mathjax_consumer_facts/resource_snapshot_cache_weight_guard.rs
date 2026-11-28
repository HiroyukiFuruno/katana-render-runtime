use super::{ident, method_call, ungroup};
use syn::Expr;

const SNAPSHOT: u8 = 1;
const PATH: u8 = 1 << 1;
const OVERHEAD: u8 = 1 << 2;
const ALL: u8 = SNAPSHOT | PATH | OVERHEAD;

pub(super) fn valid_weight(expression: &Expr, snapshot: &str, path: &str) -> bool {
    weight_terms(expression, snapshot, path) == Some(ALL)
}

fn weight_terms(expression: &Expr, snapshot: &str, path: &str) -> Option<u8> {
    if valid_length(expression, snapshot, "bytes") {
        return Some(SNAPSHOT);
    }
    if valid_length(expression, path, "as_os_str") {
        return Some(PATH);
    }
    if ident(expression, "ENTRY_OVERHEAD") {
        return Some(OVERHEAD);
    }
    let Expr::MethodCall(add) = ungroup(expression) else {
        return None;
    };
    if add.method != "saturating_add" || add.args.len() != 1 {
        return None;
    }
    let left = weight_terms(&add.receiver, snapshot, path)?;
    let right = weight_terms(&add.args[0], snapshot, path)?;
    (left & right == 0).then_some(left | right)
}

fn valid_length(expression: &Expr, receiver: &str, getter: &str) -> bool {
    let Some(length) = method_call(ungroup(expression)) else {
        return false;
    };
    length.method == "len"
        && length.args.is_empty()
        && matches!(method_call(ungroup(&length.receiver)), Some(inner)
            if inner.method == getter && inner.args.is_empty() && ident(&inner.receiver, receiver))
}
