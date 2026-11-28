use resvg::usvg::fontdb::Database;
use std::{cell::RefCell, rc::Rc};

type Inventory = Option<Rc<Database>>;

thread_local! {
    static FRAME_INVENTORY: RefCell<Option<Inventory>> = const { RefCell::new(None) };
}

pub(in crate::markdown::svg_rasterize) struct FontInventoryScope(Option<Inventory>);

impl FontInventoryScope {
    pub(in crate::markdown::svg_rasterize) fn enter() -> Self {
        /* WHY: 書体一覧は同じフレームだけで共有し、入れ子描画と次のフレームは改めて観測する。 */
        Self(FRAME_INVENTORY.with(|active| active.replace(Some(None))))
    }
}

impl Drop for FontInventoryScope {
    fn drop(&mut self) {
        FRAME_INVENTORY.with(|active| {
            let _ = active.replace(self.0.take());
        });
    }
}

pub(super) fn frame_font_inventory(load: impl FnOnce() -> Database) -> Option<Rc<Database>> {
    let cached = FRAME_INVENTORY.with_borrow(Clone::clone)?;
    Some(cached.unwrap_or_else(|| cache_inventory(load())))
}

fn cache_inventory(database: Database) -> Rc<Database> {
    let database = Rc::new(database);
    FRAME_INVENTORY.with_borrow_mut(|active| {
        if let Some(active) = active {
            *active = Some(Rc::clone(&database));
        }
    });
    database
}

#[cfg(test)]
mod tests {
    use super::{FontInventoryScope, frame_font_inventory};
    use resvg::usvg::fontdb::Database;
    use std::{
        cell::Cell,
        panic::{catch_unwind, resume_unwind},
        rc::Rc,
    };

    #[test]
    fn shares_only_inside_one_frame_and_restores_the_outer_frame() -> Result<(), &'static str> {
        assert!(frame_font_inventory(Database::new).is_none());
        let loads = Cell::new(0);
        let load = || {
            loads.set(loads.get() + 1);
            Database::new()
        };
        let outer = FontInventoryScope::enter();
        let original = frame_font_inventory(load).ok_or("missing outer inventory")?;
        let reused = frame_font_inventory(load).ok_or("missing reused inventory")?;
        assert!(Rc::ptr_eq(&original, &reused));
        assert_eq!(loads.get(), 1);
        {
            let _inner = FontInventoryScope::enter();
            let distinct = frame_font_inventory(load).ok_or("missing nested inventory")?;
            assert!(!Rc::ptr_eq(&original, &distinct));
        }
        let restored = frame_font_inventory(load).ok_or("missing restored inventory")?;
        assert!(Rc::ptr_eq(&original, &restored));
        drop(outer);
        assert!(frame_font_inventory(load).is_none());
        assert_eq!(loads.get(), 2);
        Ok(())
    }

    #[test]
    fn aborted_nested_frame_restores_the_outer_inventory() -> Result<(), &'static str> {
        let _outer = FontInventoryScope::enter();
        let original = frame_font_inventory(Database::new).ok_or("missing outer inventory")?;
        let aborted = catch_unwind(|| {
            let _inner = FontInventoryScope::enter();
            let _ = frame_font_inventory(Database::new);
            resume_unwind(Box::new("abort nested frame"));
        });
        assert!(aborted.is_err());
        let restored = frame_font_inventory(Database::new).ok_or("missing restored inventory")?;
        assert!(Rc::ptr_eq(&original, &restored));
        Ok(())
    }
}
