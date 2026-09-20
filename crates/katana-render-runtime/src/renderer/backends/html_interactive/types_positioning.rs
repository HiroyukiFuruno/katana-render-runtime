#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ElementPositioningContext {
    InFlow,
    FixedViewport,
    AbsoluteContainingBlock {
        owner_node_id: Option<u64>,
        viewport_escape: bool,
    },
}

impl ElementPositioningContext {
    pub(crate) fn escapes_element_root(self) -> bool {
        matches!(
            self,
            Self::FixedViewport
                | Self::AbsoluteContainingBlock {
                    viewport_escape: true,
                    ..
                }
        )
    }
}
