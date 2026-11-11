use crate::renderer::backends::html_css_rule::{CssDeclaration, parse_declarations};
use std::{
    cell::RefCell, collections::VecDeque, marker::PhantomData, mem::size_of_val, rc::Rc, sync::Arc,
};

const MAX_ENTRIES: usize = 256;
const MAX_SOURCE_BYTES: usize = 16 * 1024;
const MAX_RETAINED_BYTES: usize = 1024 * 1024;

struct Entry {
    source: String,
    declarations: Arc<[CssDeclaration]>,
    retained_bytes: usize,
}

#[derive(Default)]
struct Cache {
    entries: VecDeque<Entry>,
    retained_bytes: usize,
}

thread_local! {
    static ACTIVE: RefCell<Option<Cache>> = const { RefCell::new(None) };
}

#[cfg(test)]
#[path = "style_declaration_cache_tests.rs"]
mod tests;

pub(super) struct Scope {
    previous: Option<Cache>,
    owns_cache: bool,
    _thread_owned: PhantomData<Rc<()>>,
}

impl Scope {
    pub(super) fn enter() -> Self {
        let previous = ACTIVE.with(|active| active.replace(Some(Cache::default())));
        Self {
            previous,
            owns_cache: true,
            _thread_owned: PhantomData,
        }
    }

    pub(super) fn enter_if_absent() -> Self {
        let already_active = ACTIVE.with(|active| active.borrow().is_some());
        if already_active {
            return Self {
                previous: None,
                owns_cache: false,
                _thread_owned: PhantomData,
            };
        }
        Self::enter()
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if self.owns_cache {
            ACTIVE.with(|active| active.replace(self.previous.take()));
            self.owns_cache = false;
        }
    }
}

pub(super) fn get_or_parse(source: &str) -> Arc<[CssDeclaration]> {
    if source.len() > MAX_SOURCE_BYTES {
        return parse_uncached(source);
    }
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(cache) = active.as_mut() else {
            return parse_uncached(source);
        };
        if let Some(declarations) = lookup(cache, source) {
            return declarations;
        }
        let declarations = parse_uncached(source);
        let source = source.to_string();
        let retained_bytes = retained_bytes(&source, &declarations);
        if !reserve(cache, retained_bytes) {
            return declarations;
        }
        cache.entries.push_back(Entry {
            source,
            declarations: Arc::clone(&declarations),
            retained_bytes,
        });
        declarations
    })
}

fn parse_uncached(source: &str) -> Arc<[CssDeclaration]> {
    Arc::from(parse_declarations(source).into_boxed_slice())
}

fn lookup(cache: &Cache, source: &str) -> Option<Arc<[CssDeclaration]>> {
    cache
        .entries
        .iter()
        .find(|entry| entry.source == source)
        .map(|entry| Arc::clone(&entry.declarations))
}

fn retained_bytes(source: &String, declarations: &[CssDeclaration]) -> usize {
    source.capacity()
        + size_of_val(declarations)
        + declarations
            .iter()
            .map(|declaration| declaration.name.capacity() + declaration.value.capacity())
            .sum::<usize>()
}

fn reserve(cache: &mut Cache, retained_bytes: usize) -> bool {
    if retained_bytes > MAX_RETAINED_BYTES {
        return false;
    }
    /* WHY: 保持量はキュー内の合計で、空になるまでに上限以下の新規項目を格納できる。 */
    while (cache.entries.len() >= MAX_ENTRIES
        || cache.retained_bytes.saturating_add(retained_bytes) > MAX_RETAINED_BYTES)
        && let Some(entry) = cache.entries.pop_front()
    {
        cache.retained_bytes -= entry.retained_bytes;
    }
    cache.retained_bytes += retained_bytes;
    true
}
