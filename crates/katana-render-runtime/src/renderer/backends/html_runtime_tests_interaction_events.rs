use super::super::render;

#[test]
fn accepts_standard_event_names_and_rejects_non_listener_values() {
    assert!(
        render("<button id=action>Run</button><script>document.getElementById('action').addEventListener('mouseover', () => {});</script>")
            .is_ok()
    );
    assert!(matches!(
        render("<button id=action>Run</button><script>document.getElementById('action').addEventListener('click', 'not a function');</script>"),
        Err(message) if message.contains("Event listener must be a function or EventListener object")
    ));
}

#[test]
fn reports_dom_bridge_errors_from_native_callbacks() {
    assert!(matches!(
        render("<div id=host></div><script>document.getElementById('host').appendChild({});</script>"),
        Err(message) if message.contains("invalid HTML node id")
    ));
    assert!(matches!(
        render("<button id=action>Run</button><script>__krr_dom('eventPath', 'not-a-node');</script>"),
        Err(message) if message.contains("invalid HTML node id: not-a-node")
    ));
    assert!(matches!(
        render("<main id=host></main><script>document.getElementById('host').insertAdjacentHTML('middle', '<p>x</p>');</script>"),
        Err(message) if message.contains("unsupported insertAdjacentHTML position: middle")
    ));
}

#[test]
fn element_scoped_queries_children_and_html_insertion_follow_dom_contract() {
    let output = render(
        r#"<section id=scope><a id=first class=link href="/first">First</a><a id=last class=link href="/last">Last</a></section><script>
const scope = document.getElementById('scope');
const failures = [];
if (scope.querySelector('#scope') !== null) failures.push('query included scope');
if (scope.querySelector('.link') !== scope.firstElementChild) failures.push('first child');
if (scope.querySelectorAll('.link').length !== 2) failures.push('query all');
if (scope.querySelectorAll(':unsupported').length !== 0) failures.push('invalid selector');
if (scope.lastElementChild.href !== '/last') failures.push('last child href');
const aside = document.createElement('aside');
aside.innerHTML = '<b id="inserted">Info</b>';
if (aside.querySelector('#inserted') !== aside.firstElementChild) failures.push('detached query');
document.body.insertAdjacentHTML('afterBegin', aside.outerHTML);
if (failures.length) {
  throw new Error(failures.join(','));
}
</script>"#,
    );

    assert!(
        matches!(&output, Ok(output) if output.starts_with("<aside><b id=\"inserted\">Info</b></aside>")),
        "{output:?}"
    );
    assert!(
        matches!(&output, Ok(output) if output.contains("<section id=\"scope\">")),
        "{output:?}"
    );
}

#[test]
fn insert_adjacent_text_keeps_markup_like_input_as_text() {
    let output = render(
        r#"<section id=host>before</section><script>
document.getElementById('host').insertAdjacentText('beforeend', '<b>literal</b>');
</script>"#,
    );

    assert!(
        matches!(&output, Ok(output) if output.contains("<section id=\"host\">before&lt;b&gt;literal&lt;/b&gt;</section>") && !output.contains("<b>literal</b>")),
        "{output:?}"
    );
}

#[test]
fn reports_empty_list_for_unsupported_selector_on_element_scope_contract() {
    let output = render(
        r#"<section id=scope><a id=first class=link href="/first">First</a><a id=last class=link href="/last">Last</a></section><script>
const scope = document.getElementById('scope');
const supported = scope.querySelector('.link') === scope.firstElementChild;
const count = scope.querySelectorAll('.link').length;
const unsupported = scope.querySelectorAll(':unsupported').length;
if (!supported) throw new Error('supported query mismatch');
if (count !== 2) throw new Error('supported queryAll mismatch');
if (unsupported !== 0) throw new Error('unsupported selector mismatch');
document.getElementById('scope').insertAdjacentHTML('afterBegin', '<span id="marker"></span>');
</script>"#,
    );

    assert!(
        matches!(&output, Ok(output) if output.starts_with("<section id=\"scope\"><span id=\"marker\"></span><a id=\"first\" class=\"link\" href=\"/first\">First</a><a id=\"last\" class=\"link\" href=\"/last\">Last</a></section>")),
        "{output:?}"
    );
    assert!(
        matches!(&output, Ok(output) if output.contains("id=\"scope\"")),
        "{output:?}"
    );
}
