use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use rstest::rstest;

use super::{Cache, NonEmptyString, collect_source_ids, svgtree};

#[path = "svg_engine_converter_node_boundary_tests.rs"]
mod node_boundary_tests;

type Generator = fn(&mut Cache) -> NonEmptyString;

const CONSECUTIVE_COLLISIONS: usize = 3;

fn cache_with_source_ids(prefix: &str, count: usize) -> Result<Cache, String> {
    let elements: String = (1..=count)
        .map(|index| format!("<linearGradient id='{prefix}{index}'/>"))
        .collect();
    let source = format!("<svg xmlns='http://www.w3.org/2000/svg'><defs>{elements}</defs></svg>");
    let xml = roxmltree::Document::parse(&source).map_err(|error| error.to_string())?;
    let document = svgtree::Document::parse_tree(&xml, None).map_err(|error| error.to_string())?;
    let fontdb = Arc::new(fontdb::Database::new());
    let shared = Rc::new(RefCell::new(fontdb.clone()));
    let mut cache = Cache::new(fontdb, shared);
    collect_source_ids(&document, &mut cache);
    Ok(cache)
}

fn assert_generator(prefix: &str, generate: Generator, collisions: usize) -> Result<(), String> {
    let mut cache = cache_with_source_ids(prefix, collisions)?;
    let first = generate(&mut cache);
    let second = generate(&mut cache);
    assert!(!first.get().is_empty());
    assert!(!second.get().is_empty());
    assert_eq!(first.get(), format!("{prefix}{}", collisions + 1));
    assert_eq!(second.get(), format!("{prefix}{}", collisions + 2));
    assert_ne!(first.get(), second.get());
    for index in 1..=collisions {
        let source_id = format!("{prefix}{index}");
        assert_ne!(first.get(), source_id);
        assert_ne!(second.get(), source_id);
    }
    Ok(())
}

#[rstest]
#[case::linear_gradient("linearGradient", Cache::gen_linear_gradient_id)]
#[case::radial_gradient("radialGradient", Cache::gen_radial_gradient_id)]
#[case::pattern("pattern", Cache::gen_pattern_id)]
#[case::image("image", Cache::gen_image_id)]
fn generated_ids_skip_collected_source_collisions_and_remain_unique(
    #[case] prefix: &str,
    #[case] generate: Generator,
    #[values(0, 1, CONSECUTIVE_COLLISIONS)] collisions: usize,
) -> Result<(), String> {
    assert_generator(prefix, generate, collisions)
}
