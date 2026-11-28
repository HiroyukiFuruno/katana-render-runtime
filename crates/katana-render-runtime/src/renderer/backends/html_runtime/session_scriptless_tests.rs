use super::{StaticHtmlRuntime, StaticHtmlRuntimeSession};
use crate::renderer::backends::html_browser::HtmlBrowserSource;
use crate::renderer::backends::html_runtime::types::HtmlRuntimeEvent;

#[test]
fn native_dom_and_navigation_work_without_creating_an_isolate() -> Result<(), String> {
    let html = "<a id=link href='#end'>Jump</a><input id=field><p id=end>End</p>";
    let mut session = start(html)?;
    assert!(session.isolate.is_none());

    let field = session
        .node_for_element_id("field")
        .ok_or("missing field")?;
    session.set_value(field.0, "updated").map_err(test_error)?;
    assert_scriptless_navigation(&mut session)?;
    assert!(!session.advance_timers(100.0).map_err(test_error)?);
    assert!(
        !session
            .update_layout_metrics(620.0, 900.0, 0.0, [])
            .map_err(test_error)?
    );
    assert!(session.isolate.is_none());
    Ok(())
}

#[test]
fn scriptless_sessions_keep_loaded_stylesheets_and_native_controls() -> Result<(), String> {
    let css = "data:text/css,p%7Bcolor%3Ared%7D";
    let html = format!(
        "<link rel=stylesheet href='{css}'><input id=choice type=checkbox><details id=panel>Details</details>"
    );
    let mut session = start(&html)?;
    assert!(session.isolate.is_none());
    assert_eq!(
        session.external_stylesheets.get(css).map(String::as_str),
        Some("p{color:red}")
    );
    let choice = session
        .node_for_element_id("choice")
        .ok_or("missing choice")?;
    let panel = session
        .node_for_element_id("panel")
        .ok_or("missing panel")?;
    assert!(session.toggle_checked(choice.0).map_err(test_error)?);
    assert!(session.toggle_open(panel.0).map_err(test_error)?);
    let markup = session.snapshot().map_err(test_error)?;
    assert!(markup.contains(r#"checked="""#));
    assert!(markup.contains(r#"open="""#));
    Ok(())
}

#[test]
fn scriptless_observation_keeps_the_real_session_in_sync() -> Result<(), String> {
    let mut session = start("<input id=field value=initial>")?;
    let field = session
        .node_for_element_id("field")
        .ok_or("missing field")?;
    let expression = "[document.querySelector('#field').value, document.activeElement?.id, location.hash, window.innerWidth]";
    assert_eq!(
        session.observe_feature_state("document.querySelector('#field').value")?,
        serde_json::json!("initial")
    );
    session.set_value(field.0, "updated").map_err(test_error)?;
    session
        .set_active_element(Some(field.0))
        .map_err(test_error)?;
    session
        .update_document_url("https://example.test/document#field")
        .map_err(test_error)?;
    assert!(
        !session
            .update_layout_metrics(620.0, 900.0, 0.0, [])
            .map_err(test_error)?
    );
    assert_eq!(
        session.observe_feature_state(expression)?,
        serde_json::json!(["updated", "field", "#field", 620])
    );
    assert!(session.isolate.is_none());
    Ok(())
}

#[test]
fn scriptless_observation_reports_an_invalid_initial_url() -> Result<(), String> {
    let mut session = start("<p>Plain</p>")?;
    let observer = session
        .scriptless_observer
        .as_mut()
        .ok_or("missing observer")?;
    "invalid URL".clone_into(&mut observer.document_url);

    let result = session.observe_feature_state("document.readyState");
    assert!(matches!(result, Err(message) if message.contains("invalid document URL")));
    assert!(session.isolate.is_none());
    Ok(())
}

#[test]
fn scriptless_observation_reports_a_focus_synchronization_failure() -> Result<(), String> {
    let mut session = start("<p>Plain</p>")?;
    assert_eq!(
        session.observe_feature_state("document.readyState")?,
        serde_json::json!("complete")
    );
    let replace =
        "(globalThis.__krrSetFocusedNodeId = () => { throw new Error('focus sync'); }, true)";
    assert_eq!(
        session.observe_feature_state(replace)?,
        serde_json::json!(true)
    );

    let result = session.observe_feature_state("document.readyState");
    assert!(matches!(result, Err(message) if message.contains("focus sync")));
    assert!(session.isolate.is_none());
    Ok(())
}

fn start(html: &str) -> Result<StaticHtmlRuntimeSession, String> {
    let source =
        HtmlBrowserSource::new(html, "https://example.test/document").map_err(test_error)?;
    StaticHtmlRuntime
        .start_interactive(&source)
        .map_err(test_error)
}

fn assert_scriptless_navigation(session: &mut StaticHtmlRuntimeSession) -> Result<(), String> {
    let target = session.node_for_element_id("link").ok_or("missing link")?;
    let (dispatch, prevented) = session
        .dispatch_with_default_prevented(HtmlRuntimeEvent::Click { target })
        .map_err(test_error)?;
    assert!(dispatch.content.contains(r#"value="updated""#));
    assert_eq!(
        dispatch.navigation.map(|intent| intent.href).as_deref(),
        Some("#end")
    );
    assert!(!prevented);
    Ok(())
}

fn test_error(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
