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

#[cfg(test)]
mod tests {
    use super::ElementPositioningContext;

    #[test]
    fn containing_block_owner_returns_owner_for_positioned_contexts() {
        assert_eq!(
            ElementPositioningContext::FixedContainingBlock {
                owner_node_id: 7,
                viewport_escape: false,
            }
            .containing_block_owner(),
            Some(7)
        );
        assert_eq!(
            ElementPositioningContext::AbsoluteContainingBlock {
                owner_node_id: Some(11),
                viewport_escape: true,
            }
            .containing_block_owner(),
            Some(11)
        );
    }

    #[test]
    fn viewport_contexts_have_no_containing_block_owner() {
        assert_eq!(
            ElementPositioningContext::InFlow.containing_block_owner(),
            None
        );
        assert_eq!(
            ElementPositioningContext::FixedViewport.containing_block_owner(),
            None
        );
        assert_eq!(
            ElementPositioningContext::AbsoluteContainingBlock {
                owner_node_id: None,
                viewport_escape: false,
            }
            .containing_block_owner(),
            None
        );
    }
}
