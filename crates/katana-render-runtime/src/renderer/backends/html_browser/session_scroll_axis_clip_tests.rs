use self::scroll_axis_clip_support::{
    AxisCase, BLUE, CASES, HTML_TEMPLATE, LIME, MAGENTA, RED, SOURCE_ORIGIN, VIEWPORT_HEIGHT,
    VIEWPORT_WIDTH, WHITE,
};
use super::scroll_metrics_tests::pixel_at_content_css;
use super::{HtmlBrowserInput, HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport};

const RATIO_DEBUG_X: f32 = 5.5;
const INTERSECTION_DEBUG_X: f32 = 7.5;
const ROOT_DEBUG_X: f32 = 9.5;
const DEBUG_Y: f32 = 5.5;
const OBSERVER_PIXEL_X: f32 = 10.0;
const OBSERVER_PIXEL_Y: f32 = 10.0;
const CLICK_OFFSET: f32 = 5.0;
const CLICK_STATE_X: f32 = 25.0;

#[path = "session_scroll_axis_clip_support.rs"]
mod scroll_axis_clip_support;

type TestResult<T = ()> = Result<T, String>;

#[test]
fn nested_one_axis_clip_preserves_visible_pixels_click_bounds_and_observer_state() -> TestResult {
    for case in CASES {
        for scale in [1.0_f32, 2.0_f32] {
            let mut session = open_axis_case(&case, scale)?;
            session
                .refresh_frame()
                .map_err(|error| format!("{}: refresh: {error}", case.name))?;
            assert_axis_metrics(&session, &case)?;
            assert_axis_pixels(&session, &case)?;
            assert_axis_observers(&session, &case)?;
            dispatch_click(
                &mut session,
                case.click_left as f32 + CLICK_OFFSET,
                case.click_top as f32 + CLICK_OFFSET,
                scale,
            )?;
            assert_eq!(
                pixel_at_content_css(
                    &session,
                    VIEWPORT_WIDTH - OBSERVER_PIXEL_X,
                    VIEWPORT_HEIGHT - OBSERVER_PIXEL_Y,
                )?,
                BLUE,
                "{} DPR {scale}: hit testing must block the clipped-axis target",
                case.name
            );
        }
    }
    Ok(())
}

fn axis_html(case: &AxisCase) -> String {
    let replacements = [
        ("__PARENT_WIDTH__", case.parent_width.to_string()),
        ("__PARENT_HEIGHT__", case.parent_height.to_string()),
        ("__OVERFLOW_X__", case.overflow_x.to_owned()),
        ("__OVERFLOW_Y__", case.overflow_y.to_owned()),
        ("__CHILD_LEFT__", case.child_left.to_string()),
        ("__CHILD_TOP__", case.child_top.to_string()),
        ("__CHILD_WIDTH__", case.child_width.to_string()),
        ("__CHILD_HEIGHT__", case.child_height.to_string()),
        ("__ACTIVE_LEFT__", case.active_left.to_string()),
        ("__ACTIVE_TOP__", case.active_top.to_string()),
        ("__CLIPPED_LEFT__", case.clipped_left.to_string()),
        ("__CLIPPED_TOP__", case.clipped_top.to_string()),
        ("__CLICK_LEFT__", case.click_left.to_string()),
        ("__CLICK_TOP__", case.click_top.to_string()),
        (
            "__EXPECTED_INTERSECTION_RATIO__",
            case.expected_intersection_ratio.to_string(),
        ),
    ];
    let mut html = HTML_TEMPLATE.to_owned();
    for (token, value) in replacements {
        html = html.replace(token, &value);
    }
    html
}

fn open_axis_case(case: &AxisCase, scale: f32) -> TestResult<HtmlBrowserSession> {
    let source = HtmlBrowserSource::new(axis_html(case), SOURCE_ORIGIN)
        .map_err(|error| error.to_string())?;
    let viewport = HtmlBrowserViewport::new(
        (VIEWPORT_WIDTH * scale) as u32,
        (VIEWPORT_HEIGHT * scale) as u32,
        scale,
    )
    .map_err(|error| error.to_string())?;
    HtmlBrowserSession::new(source, viewport).map_err(|error| error.to_string())
}

fn assert_axis_metrics(session: &HtmlBrowserSession, case: &AxisCase) -> TestResult {
    let (_, metrics) = session
        .latest_frame_with_metrics()
        .ok_or_else(|| format!("{}: frame and metrics must be paired", case.name))?;
    assert_eq!(metrics.viewport_width, VIEWPORT_WIDTH, "{}", case.name);
    assert_eq!(metrics.viewport_height, VIEWPORT_HEIGHT, "{}", case.name);
    assert_content_metrics(metrics, case)?;
    assert_visible_axis_ratio(metrics, case)
}

fn assert_content_metrics(
    metrics: &super::super::HtmlBrowserScrollMetrics,
    case: &AxisCase,
) -> TestResult {
    assert_eq!(
        metrics.content_width, case.expected_content_width,
        "{}",
        case.name
    );
    assert_eq!(
        metrics.content_height, case.expected_content_height,
        "{}",
        case.name
    );
    Ok(())
}

fn assert_visible_axis_ratio(
    metrics: &super::super::HtmlBrowserScrollMetrics,
    case: &AxisCase,
) -> TestResult {
    let (viewport_extent, visible_extent) = if case.overflow_x == "clip" {
        (metrics.viewport_height, metrics.content_height)
    } else {
        (metrics.viewport_width, metrics.content_width)
    };
    assert_eq!(
        viewport_extent / visible_extent,
        case.visible_axis_ratio,
        "{}",
        case.name
    );
    Ok(())
}

fn assert_axis_pixels(session: &HtmlBrowserSession, case: &AxisCase) -> TestResult {
    assert_eq!(
        pixel_at_content_css(
            session,
            case.visible_pixel.0 as f32,
            case.visible_pixel.1 as f32
        )?,
        RED,
        "{}: visible-axis overflow must remain painted past the rounded parent",
        case.name
    );
    assert_eq!(
        pixel_at_content_css(
            session,
            case.clipped_pixel.0 as f32,
            case.clipped_pixel.1 as f32
        )?,
        WHITE,
        "{}: clipped-axis overflow must not paint past the parent",
        case.name
    );
    Ok(())
}

fn assert_axis_observers(session: &HtmlBrowserSession, case: &AxisCase) -> TestResult {
    let diagnostics = observer_diagnostics(session)?;
    assert_eq!(
        pixel_at_content_css(
            session,
            OBSERVER_PIXEL_X,
            VIEWPORT_HEIGHT - OBSERVER_PIXEL_Y
        )?,
        LIME,
        "{}: child IO ratio should be {} after active-axis clipping; diagnostics={diagnostics}",
        case.name,
        case.expected_intersection_ratio
    );
    assert_eq!(
        pixel_at_content_css(session, OBSERVER_PIXEL_X, OBSERVER_PIXEL_Y)?,
        LIME,
        "{}: IntersectionObserver must keep the visible-axis target active",
        case.name
    );
    assert_eq!(
        pixel_at_content_css(session, CLICK_STATE_X, OBSERVER_PIXEL_Y)?,
        MAGENTA,
        "{}: IntersectionObserver must exclude the clipped-axis target",
        case.name
    );
    Ok(())
}

fn observer_diagnostics(session: &HtmlBrowserSession) -> TestResult<String> {
    let ratio = pixel_at_content_css(session, VIEWPORT_WIDTH - RATIO_DEBUG_X, DEBUG_Y)?;
    let intersection =
        pixel_at_content_css(session, VIEWPORT_WIDTH - INTERSECTION_DEBUG_X, DEBUG_Y)?;
    let root = pixel_at_content_css(session, VIEWPORT_WIDTH - ROOT_DEBUG_X, DEBUG_Y)?;
    Ok(format!(
        "ratioRgb={ratio:?}, intersectionRgb={intersection:?}, rootRgb={root:?}"
    ))
}

fn dispatch_click(session: &mut HtmlBrowserSession, x: f32, y: f32, scale: f32) -> TestResult {
    for input in [
        HtmlBrowserInput::PointerDown {
            x: x * scale,
            y: y * scale,
            button: 0,
        },
        HtmlBrowserInput::PointerUp {
            x: x * scale,
            y: y * scale,
            button: 0,
        },
    ] {
        session
            .dispatch_input(input)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
