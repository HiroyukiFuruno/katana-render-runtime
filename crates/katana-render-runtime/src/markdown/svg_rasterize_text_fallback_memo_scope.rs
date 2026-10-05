use super::entries::SelectorMemo;
use std::cell::RefCell;

thread_local! {
    pub(super) static ACTIVE_MEMO: RefCell<Option<SelectorMemo>> = const { RefCell::new(None) };
}

struct MemoScope {
    previous: Option<SelectorMemo>,
}

impl MemoScope {
    fn enter() -> Self {
        /* WHY: 入れ子のparseで外側の選択結果を共有しない。 */
        let previous = ACTIVE_MEMO.with(|active| active.replace(None));
        if super::super::font::memo_usable() {
            ACTIVE_MEMO.with(|active| active.replace(Some(SelectorMemo::default())));
        }
        Self { previous }
    }
}

impl Drop for MemoScope {
    fn drop(&mut self) {
        ACTIVE_MEMO.with(|active| active.replace(self.previous.take()));
    }
}

pub(super) fn with_attempt<T>(operation: impl FnOnce() -> T) -> T {
    let _scope = MemoScope::enter();
    operation()
}
