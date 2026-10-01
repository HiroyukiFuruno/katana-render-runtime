use super::{TestResult, render};

#[test]
fn later_duplicate_body_onload_survives_earlier_attribute_removal() -> TestResult {
    let output = render(
        r#"<body onload="document.getElementById('status').textContent = 'first-body'"><p id=status>Waiting</p><script>document.body.removeAttribute('onload'); globalThis.__krrInstallStaticBodyLoadHandler = () => {};</script><body onload="document.getElementById('status').textContent = 'later-body'">"#,
    )?;

    assert!(output.contains(">later-body</p>"), "{output}");
    Ok(())
}

#[test]
fn later_duplicate_body_onload_uses_native_dom_after_method_overrides() -> TestResult {
    let output = render(
        r#"<body><p id=status>Waiting</p><script>document.body.removeAttribute('onload'); document.body.getAttribute = () => null; document.body.setAttribute = () => {};</script><body onload="document.getElementById('status').textContent = 'later-body'">"#,
    )?;

    assert!(output.contains(">later-body</p>"), "{output}");
    Ok(())
}

#[test]
fn removed_earlier_body_onload_is_not_restored_by_duplicate_body_without_onload() -> TestResult {
    let output = render(
        r#"<body onload="document.getElementById('status').textContent = 'old-body'"><p id=status>Waiting</p><script>document.body.removeAttribute('onload');</script><body>"#,
    )?;

    assert!(output.contains(">Waiting</p>"), "{output}");
    assert!(!output.contains(">old-body</p>"), "{output}");
    Ok(())
}

#[test]
fn earlier_body_onload_mutation_survives_later_duplicate_body_token() -> TestResult {
    let output = render(
        r#"<body><p id=status>Waiting</p><script>document.body.setAttribute('onload', "document.getElementById('status').textContent = 'script-body'");</script><body onload="document.getElementById('status').textContent = 'token-body'">"#,
    )?;

    assert!(output.contains(">script-body</p>"), "{output}");
    assert!(!output.contains(">token-body</p>"), "{output}");
    Ok(())
}

const LATER_BODY_ONLOAD_CASES: &[(&str, &str, &str)] = &[
    (
        r#""#,
        r#"<body onload="document.getElementById('status').textContent = 'second'">"#,
        "first",
    ),
    (
        r#"window.onload = () => document.getElementById('status').textContent = 'author';"#,
        r#"<body onload="document.getElementById('status').textContent = 'first'">"#,
        "author",
    ),
    (
        r#"document.body.removeAttribute('onload'); document.body.setAttribute('onload', "document.getElementById('status').textContent = 'attribute'");"#,
        r#"<body onload="document.getElementById('status').textContent = 'second'">"#,
        "attribute",
    ),
    (
        r#"document.body.removeAttribute('onload');"#,
        r#"<body onload="document.getElementById('status').textContent = 'second'"><script>document.body.removeAttribute('onload');</script><body onload="document.getElementById('status').textContent = 'third'">"#,
        "third",
    ),
    (
        r#"document.body.removeAttribute('onload');"#,
        r#"<template><body onload="document.getElementById('status').textContent = 'second'"></template><select><body onload="document.getElementById('status').textContent = 'second'"></select>"#,
        "Waiting",
    ),
    (
        r#"document.body.removeAttribute('onload'); document.body.getAttribute = () => 'spoof'; document.body.setAttribute = () => {}; Object.defineProperty(document, 'body', { get: () => null }); document.querySelector = () => null; globalThis.__krr_dom = () => null;"#,
        r#"<body onload="document.getElementById('status').textContent = 'second'">"#,
        "second",
    ),
    (
        r#"const savedFunction = Function; const savedString = String; const savedApply = Reflect.apply; const statusCaptured = document.getElementById('status'); document.body.removeAttribute('onload'); globalThis.__krrInstallStaticBodyLoadHandler = () => {}; globalThis.Function = () => null; globalThis.String = () => 'spoof'; Reflect.apply = () => null;"#,
        r#"<body onload="statusCaptured.textContent = 'second'"><script>globalThis.Function = savedFunction; globalThis.String = savedString; Reflect.apply = savedApply;</script>"#,
        "second",
    ),
    (
        r#"const savedMapGet = Map.prototype.get; const savedMapSet = Map.prototype.set; const savedMapDelete = Map.prototype.delete; const savedWeakGet = WeakMap.prototype.get; const savedWeakSet = WeakMap.prototype.set; const statusCaptured = document.getElementById('status'); document.body.removeAttribute('onload'); Map.prototype.get = () => undefined; Map.prototype.set = () => {}; Map.prototype.delete = () => {}; WeakMap.prototype.get = () => undefined; WeakMap.prototype.set = () => {};"#,
        r#"<body onload="statusCaptured.textContent = 'second'"><script>Map.prototype.get = savedMapGet; Map.prototype.set = savedMapSet; Map.prototype.delete = savedMapDelete; WeakMap.prototype.get = savedWeakGet; WeakMap.prototype.set = savedWeakSet;</script>"#,
        "second",
    ),
    (
        r#"document.body.removeAttribute('onload'); window.authorCalls = 0; globalThis.__krrInstallStaticBodyLoadHandler = () => { window.authorCalls += 1; };"#,
        r#"<body onload="document.getElementById('status').textContent = `second:${window.authorCalls}`"><script>__krrInstallStaticBodyLoadHandler('author-source', true);</script>"#,
        "second:1",
    ),
];

#[test]
fn later_duplicate_body_onload_preserves_live_handlers_and_parser_order() -> TestResult {
    let first = "document.getElementById('status').textContent = 'first'";
    for (middle, later, expected) in LATER_BODY_ONLOAD_CASES {
        let source = format!(
            r#"<body onload="{first}"><p id=status>Waiting</p><script>{middle}</script>{later}"#
        );
        let output = render(&source)?;
        assert!(
            output.contains(&format!(">{expected}</p>")),
            "{source}: {output}"
        );
    }
    Ok(())
}
