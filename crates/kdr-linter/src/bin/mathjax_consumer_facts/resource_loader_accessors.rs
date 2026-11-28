use super::super::super::source::{call, ident, method, path_name};
use syn::{Expr, Stmt};

pub(super) fn require_loaded_accessors(file: &syn::File) -> Result<(), String> {
    for (name, expected) in [("text", "text"), ("digest", "digest")] {
        let function = method(file, "LoadedResource", name)?;
        let [Stmt::Expr(Expr::Match(value), None)] = function.block.stmts.as_slice() else {
            return Err("resource-loader:accessors".into());
        };
        if !ident(&value.expr, "self.source")
            || value.arms.len() != 2
            || !matches!(&value.arms[0].pat, syn::Pat::TupleStruct(pattern)
                if path_name(&pattern.path) == "ResourceSource::Static"
                    && pattern.elems.len() == 1
                    && ((name == "text" && matches!(&pattern.elems[0], syn::Pat::Ident(binding) if binding.ident == "text")
                            && ident(&value.arms[0].body, "text"))
                        || (name == "digest" && matches!(&pattern.elems[0], syn::Pat::Wild(_))
                            && ident(&value.arms[0].body, "None"))))
            || !snapshot_arm(&value.arms[1], expected)
        {
            return Err("resource-loader:accessors".into());
        }
    }
    Ok(())
}

fn snapshot_arm(arm: &syn::Arm, name: &str) -> bool {
    let binding = if name == "digest" { "snapshot" } else { "text" };
    if !matches!(&arm.pat, syn::Pat::Struct(pattern)
        if path_name(&pattern.path) == "ResourceSource::File"
            && pattern.fields.len() == 1 && pattern.rest.is_some()
            && pattern.fields.iter().any(|field| matches!(&field.member, syn::Member::Named(value) if value == binding)
                && matches!(&*field.pat, syn::Pat::Ident(value) if value.ident == binding)))
    {
        return false;
    }
    let expression = if name == "digest" {
        let Some(outer) = call(&arm.body) else {
            return false;
        };
        if !ident(&outer.func, "Some") || outer.args.len() != 1 {
            return false;
        }
        &outer.args[0]
    } else {
        arm.body.as_ref()
    };
    matches!(super::method_call(expression), Some(value)
        if value.method == if name == "digest" { "digest" } else { "as_str" }
            && value.args.is_empty() && ident(&value.receiver, binding))
}
