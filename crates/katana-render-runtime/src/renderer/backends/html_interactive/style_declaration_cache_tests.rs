use super::super::style::{CssStyle, CssVerticalAlign};
use super::{
    Cache, MAX_ENTRIES, MAX_RETAINED_BYTES, MAX_SOURCE_BYTES, Scope, get_or_parse, reserve,
};
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::Arc;

type DeclarationSignature = Vec<(String, String, bool)>;

#[test]
fn reserve_rejects_one_entry_over_the_retained_limit_without_mutation() {
    let mut cache = Cache::default();

    assert!(!reserve(&mut cache, MAX_RETAINED_BYTES + 1));
    assert!(cache.entries.is_empty());
    assert_eq!(cache.retained_bytes, 0);
}

#[test]
fn parser_order_values_and_important_are_preserved() {
    let _scope = Scope::enter();
    let declarations = get_or_parse("color: red !important; margin-top: 1px; color: blue;");
    assert_eq!(
        signature(&declarations),
        vec![
            ("color".into(), "red".into(), true),
            ("margin-top".into(), "1px".into(), false),
            ("color".into(), "blue".into(), false),
        ]
    );
}

#[test]
fn changed_source_is_reparsed_and_same_source_is_reused() {
    let _scope = Scope::enter();
    let first = get_or_parse("font-size: 12px; color: red !important;");
    let reused = get_or_parse("font-size: 12px; color: red !important;");
    let changed = get_or_parse("font-size: 13px; color: red !important;");
    assert!(Arc::ptr_eq(&first, &reused));
    assert!(!Arc::ptr_eq(&first, &changed));
    assert_eq!(changed[0].value, "13px");
}

#[test]
fn nested_scope_unwind_restores_outer_cache() {
    let outer = Scope::enter();
    let source = "line-height: 1.4;";
    let outer_value = get_or_parse(source);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _nested = Scope::enter();
        let nested_value = get_or_parse(source);
        assert!(!Arc::ptr_eq(&outer_value, &nested_value));
        resume_unwind(Box::new("scope test unwind"));
    }));
    assert!(result.is_err());
    let restored = get_or_parse(source);
    assert!(Arc::ptr_eq(&outer_value, &restored));
    drop(outer);
}

#[test]
fn enter_if_absent_reuses_outer_cache_arc() {
    let outer = Scope::enter();
    let source = "color: green;";
    let outer_value = get_or_parse(source);
    let borrowed = Scope::enter_if_absent();
    let borrowed_value = get_or_parse(source);
    assert!(Arc::ptr_eq(&outer_value, &borrowed_value));
    drop(borrowed);
    let restored = get_or_parse(source);
    assert!(Arc::ptr_eq(&outer_value, &restored));
    drop(outer);
}

#[test]
fn enter_if_absent_reparses_changed_source_without_stale_value() {
    let outer = Scope::enter();
    let first = get_or_parse("font-size: 12px;");
    let borrowed = Scope::enter_if_absent();
    let changed = get_or_parse("font-size: 13px;");
    assert!(!Arc::ptr_eq(&first, &changed));
    assert_eq!(changed[0].value, "13px");
    drop(borrowed);
    drop(outer);
}

#[test]
fn borrowed_scope_drop_preserves_outer_entries_and_bounds() -> Result<(), String> {
    let outer = Scope::enter();
    let source = "line-height: 1.3;";
    let outer_value = get_or_parse(source);
    let borrowed = Scope::enter_if_absent();
    let borrowed_value = get_or_parse(source);
    drop(borrowed);
    let restored = get_or_parse(source);
    if !Arc::ptr_eq(&outer_value, &borrowed_value) || !Arc::ptr_eq(&outer_value, &restored) {
        return Err("borrowed scope did not preserve outer entry".to_string());
    }
    super::ACTIVE.with(|active| {
        let cache = active.borrow();
        let cache = cache
            .as_ref()
            .ok_or_else(|| "outer cache was dropped".to_string())?;
        if cache.entries.len() > MAX_ENTRIES || cache.retained_bytes > MAX_RETAINED_BYTES {
            return Err("outer cache bounds changed after borrowed drop".to_string());
        }
        Ok(())
    })?;
    drop(outer);
    Ok(())
}

#[test]
fn borrowed_scope_unwind_keeps_outer_then_fresh_scope_cleans_up() {
    let outer = Scope::enter();
    let source = "font-weight: 500;";
    let outer_value = get_or_parse(source);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _borrowed = Scope::enter_if_absent();
        let borrowed_value = get_or_parse(source);
        assert!(Arc::ptr_eq(&outer_value, &borrowed_value));
        resume_unwind(Box::new("borrowed scope unwind"));
    }));
    assert!(result.is_err());
    let restored = get_or_parse(source);
    assert!(Arc::ptr_eq(&outer_value, &restored));
    drop(outer);
    let fresh = Scope::enter();
    let fresh_value = get_or_parse(source);
    assert!(!Arc::ptr_eq(&outer_value, &fresh_value));
    drop(fresh);
}

#[test]
fn outside_scope_does_not_reuse_cached_arc() {
    let source = "font-weight: 600;";
    let first = get_or_parse(source);
    let second = get_or_parse(source);
    assert!(!Arc::ptr_eq(&first, &second));
}

#[test]
fn entry_bound_evicts_oldest_and_keeps_latest_source() -> Result<(), String> {
    let _scope = Scope::enter();
    let first_source = "--entry-0: value;".to_string();
    let first = get_or_parse(&first_source);
    let mut latest_source = String::new();
    let mut latest = None;
    for index in 1..=MAX_ENTRIES {
        latest_source = format!("--entry-{index}: value;");
        latest = Some(get_or_parse(&latest_source));
    }
    let latest = latest.ok_or_else(|| "latest source was not inserted".to_string())?;
    let reloaded_first = get_or_parse(&first_source);
    let reused_latest = get_or_parse(&latest_source);
    if Arc::ptr_eq(&first, &reloaded_first) {
        return Err("oldest source was not evicted".to_string());
    }
    if !Arc::ptr_eq(&latest, &reused_latest) {
        return Err("latest source was not retained".to_string());
    }
    super::ACTIVE.with(|active| {
        let cache = active.borrow();
        let cache = cache
            .as_ref()
            .ok_or_else(|| "test scope must activate cache".to_string())?;
        if cache.entries.len() != MAX_ENTRIES {
            return Err("entry bound was not filled exactly".to_string());
        }
        Ok(())
    })
}

#[test]
fn entry_and_retained_memory_bounds_hold_after_many_sources() -> Result<(), String> {
    let _scope = Scope::enter();
    for index in 0..(MAX_ENTRIES + 32) {
        let source = format!("--custom-{index}: {};", "x".repeat(6_000));
        let _ = get_or_parse(&source);
    }
    super::ACTIVE.with(|active| {
        let cache = active.borrow();
        let cache = cache
            .as_ref()
            .ok_or_else(|| "test scope must activate cache".to_string())?;
        if cache.entries.len() > MAX_ENTRIES {
            return Err("entry bound exceeded".to_string());
        }
        if cache.retained_bytes > MAX_RETAINED_BYTES {
            return Err("retained memory bound exceeded".to_string());
        }
        if cache.retained_bytes != recompute_retained_bytes(cache) {
            return Err("retained memory accounting drifted".to_string());
        }
        Ok(())
    })
}

#[test]
fn cached_declarations_are_applied_against_each_inherited_style() {
    let _scope = Scope::enter();
    let attributes = vec![(
        "style".to_string(),
        "font-size: 2em; vertical-align: inherit;".to_string(),
    )];
    let mut first_parent = CssStyle::browser_default();
    first_parent.font_size = 10.0;
    first_parent.vertical_align = CssVerticalAlign::Top;
    let mut second_parent = CssStyle::browser_default();
    second_parent.font_size = 18.0;
    second_parent.vertical_align = CssVerticalAlign::Bottom;
    let first = CssStyle::from_attributes(&attributes, &first_parent);
    let second = CssStyle::from_attributes(&attributes, &second_parent);
    assert_eq!(first.font_size, 20.0);
    assert_eq!(second.font_size, 36.0);
    assert_eq!(first.vertical_align, CssVerticalAlign::Top);
    assert_eq!(second.vertical_align, CssVerticalAlign::Bottom);
}

#[test]
fn oversized_source_is_parsed_without_cache_retention() -> Result<(), String> {
    let _scope = Scope::enter();
    let source = format!("--oversized: {};", "x".repeat(MAX_SOURCE_BYTES));
    let declarations = get_or_parse(&source);
    if declarations.len() != 1 {
        return Err("oversized source was not parsed".to_string());
    }
    super::ACTIVE.with(|active| {
        let cache = active.borrow();
        if cache.as_ref().map_or(0, |cache| cache.entries.len()) != 0 {
            return Err("oversized source was retained".to_string());
        }
        Ok(())
    })
}

fn signature(declarations: &[super::CssDeclaration]) -> DeclarationSignature {
    declarations
        .iter()
        .map(|declaration| {
            (
                declaration.name.clone(),
                declaration.value.clone(),
                declaration.important,
            )
        })
        .collect()
}

fn recompute_retained_bytes(cache: &super::Cache) -> usize {
    cache
        .entries
        .iter()
        .map(|entry| {
            entry.source.capacity()
                + size_of::<super::CssDeclaration>() * entry.declarations.len()
                + entry
                    .declarations
                    .iter()
                    .map(|declaration| declaration.name.capacity() + declaration.value.capacity())
                    .sum::<usize>()
        })
        .sum()
}
