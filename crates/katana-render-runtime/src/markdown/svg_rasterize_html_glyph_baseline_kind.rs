use super::PolicyKind;
use resvg::usvg::{AlignmentBaseline, DominantBaseline, TextSpan};

pub(super) fn span_kind(span: &TextSpan, middle_supported: bool) -> Option<PolicyKind> {
    let baseline = span.alignment_baseline();
    if !matches!(
        baseline,
        AlignmentBaseline::Auto | AlignmentBaseline::Baseline
    ) {
        return match baseline {
            AlignmentBaseline::Mathematical => Some(PolicyKind::Mathematical),
            AlignmentBaseline::Central => Some(PolicyKind::Central),
            AlignmentBaseline::Middle if middle_supported => Some(PolicyKind::AlignmentMiddle),
            _ => None,
        };
    }
    match span.dominant_baseline() {
        DominantBaseline::Mathematical => Some(PolicyKind::Mathematical),
        DominantBaseline::Central => Some(PolicyKind::Central),
        DominantBaseline::Hanging => Some(PolicyKind::Hanging),
        DominantBaseline::Middle if middle_supported => Some(PolicyKind::Middle),
        _ => None,
    }
}
