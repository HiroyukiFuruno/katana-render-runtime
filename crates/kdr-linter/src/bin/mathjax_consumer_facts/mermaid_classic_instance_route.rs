use super::super::super::source::{Source, ident, local, method};
use syn::Expr;

const INSTANCE_STATEMENTS: usize = 4;
const INSTANCE_TAIL_INDEX: usize = 3;

pub(crate) struct InstanceGuard;

impl InstanceGuard {
    pub(crate) fn inspect(render: &Source, instance: &Source) -> Result<(), String> {
        require_module(render)?;
        let unique = method(&instance.ast, "MermaidRenderOps", "unique_svg_instance")?;
        if unique.block.stmts.len() != INSTANCE_STATEMENTS {
            return Err("mermaid-resource-route:instance".into());
        }
        require_root(&unique.block)?;
        require_sequence(&unique.block)?;
        require_replacement(&unique.block)
    }
}

fn require_module(source: &Source) -> Result<(), String> {
    let modules = source
        .ast
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(item) if item.ident == "render_instance" => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [module] = modules.as_slice() else {
        return Err("mermaid-resource-route:instance-module".into());
    };
    if module.content.is_some()
        || module.attrs.len() != 1
        || !module.attrs[0].path().is_ident("path")
        || !matches!(&module.attrs[0].meta, syn::Meta::NameValue(value)
            if matches!(&value.value, Expr::Lit(lit) if matches!(&lit.lit, syn::Lit::Str(text)
                if text.value() == "render_instance.rs")))
    {
        return Err("mermaid-resource-route:instance-module".into());
    }
    Ok(())
}

fn require_root(block: &syn::Block) -> Result<(), String> {
    let Some(syn::Stmt::Local(value)) = block.stmts.first() else {
        return Err("mermaid-resource-route:instance".into());
    };
    if !matches!(&value.pat, syn::Pat::TupleStruct(pat) if pat.path.is_ident("Some")
        && pat.elems.len() == 1 && matches!(&pat.elems[0], syn::Pat::Ident(name) if name.ident == "root_id"))
        || !value.init.as_ref().is_some_and(|init|
            super::super::super::syntax::call_expr_path(&init.expr, "Self::root_svg_id", &["svg"])
            && matches!(&init.diverge, Some((_, expression)) if matches!(expression.as_ref(), Expr::Block(body)
                if matches!(body.block.stmts.as_slice(), [syn::Stmt::Expr(Expr::Return(value), _)]
                    if value.expr.as_deref().is_some_and(|value| ident(value, "svg"))))))
    {
        return Err("mermaid-resource-route:instance".into());
    }
    Ok(())
}

fn require_sequence(block: &syn::Block) -> Result<(), String> {
    let sequence = local(block, 1, "sequence")?;
    if !matches!(sequence, Expr::MethodCall(value) if value.method == "fetch_add"
        && ident(&value.receiver, "MERMAID_SVG_RENDER_SEQUENCE") && value.args.len() == 2
        && matches!(&value.args[0], Expr::Lit(lit) if matches!(&lit.lit, syn::Lit::Int(value) if matches!(value.base10_parse::<u8>(), Ok(1))))
        && ident(&value.args[1], "Ordering::Relaxed"))
    {
        return Err("mermaid-resource-route:instance".into());
    }
    let Expr::Macro(format) = local(block, 2, "unique_id")? else {
        return Err("mermaid-resource-route:instance".into());
    };
    if !format.mac.path.is_ident("format")
        || format.mac.tokens.to_string() != "\"{root_id}-{sequence:016x}\""
    {
        return Err("mermaid-resource-route:instance".into());
    }
    Ok(())
}

fn require_replacement(block: &syn::Block) -> Result<(), String> {
    if !matches!(block.stmts.get(INSTANCE_TAIL_INDEX), Some(syn::Stmt::Expr(Expr::MethodCall(value), _))
        if value.method == "replace" && ident(&value.receiver, "svg") && value.args.len() == 2
            && ident(&value.args[0], "root_id") && ident(&value.args[1], "unique_id"))
    {
        return Err("mermaid-resource-route:instance".into());
    }
    Ok(())
}
