#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ElementPositioningContext {
    InFlow,
    FixedViewport,
    FixedContainingBlock {
        owner_node_id: u64,
        viewport_escape: bool,
    },
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
                | Self::FixedContainingBlock {
                    viewport_escape: true,
                    ..
                }
                | Self::AbsoluteContainingBlock {
                    viewport_escape: true,
                    ..
                }
        )
    }

    pub(crate) fn containing_block_owner(self) -> Option<u64> {
        match self {
            Self::FixedContainingBlock { owner_node_id, .. } => Some(owner_node_id),
            Self::AbsoluteContainingBlock { owner_node_id, .. } => owner_node_id,
            Self::InFlow | Self::FixedViewport => None,
        }
    }
}
