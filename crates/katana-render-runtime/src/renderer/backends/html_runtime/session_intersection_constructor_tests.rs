use super::super::StaticHtmlRuntime;
use super::super::types::HtmlRuntimeError;

#[test]
fn intersection_observer_constructor_validates_margins_and_thresholds()
-> Result<(), HtmlRuntimeError> {
    let source = constructor_validation_source();
    let session = StaticHtmlRuntime.start(&source)?;
    let snapshot = session.snapshot()?;
    assert_constructor_valid_options(&snapshot);
    assert_constructor_invalid_options(&snapshot);
    Ok(())
}

fn assert_constructor_valid_options(snapshot: &str) {
    assert_constructor_attributes(
        snapshot,
        &[
            ("valid-margin", "10px 5% -3px -2%"),
            ("empty-margin", "0px 0px 0px 0px"),
            ("space-margin", "0px 0px 0px 0px"),
            ("default-margin", "0px 0px 0px 0px"),
            ("sorted-thresholds", "0,0.25,1"),
            ("empty-threshold", "0"),
            ("default-threshold", "0"),
            ("zero-threshold", "0"),
            ("one-threshold", "1"),
            ("string-threshold", "0.5"),
            ("set-thresholds", "0,0.5"),
            ("typed-array-thresholds", "0,0.5"),
            ("iterator-reads", "1"),
            ("null-root", "true"),
            ("document-root", "true"),
            ("element-root", "true"),
            ("root-registrations", "1"),
        ],
    );
}

fn assert_constructor_invalid_options(snapshot: &str) {
    assert_constructor_attributes(
        snapshot,
        &[
            ("margin-unitless", "SyntaxError"),
            ("margin-em", "SyntaxError"),
            ("margin-arity", "SyntaxError"),
            ("margin-null", "SyntaxError"),
            ("threshold-low", "RangeError"),
            ("threshold-high", "RangeError"),
            ("threshold-nan", "TypeError"),
            ("threshold-infinity", "TypeError"),
            ("threshold-negative-infinity", "TypeError"),
            ("threshold-sparse", "TypeError"),
            ("threshold-boxed-bigint", "TypeError"),
            ("threshold-iterator-error", "Error"),
            ("threshold-noncallable-iterator", "TypeError"),
            ("root-window", "TypeError"),
            ("root-object", "TypeError"),
        ],
    );
}

fn assert_constructor_attributes(snapshot: &str, expected: &[(&str, &str)]) {
    for (attribute, value) in expected {
        assert!(snapshot.contains(&format!(r#"data-{attribute}="{value}""#)));
    }
}

fn constructor_validation_source() -> String {
    [
        constructor_validation_prelude(),
        constructor_validation_success_cases(),
        constructor_validation_error_cases(),
        constructor_root_registration_cases(),
        "</script>",
    ]
    .concat()
}

fn constructor_validation_prelude() -> &'static str {
    r#"<p id="result"></p><script>
        const result = document.getElementById("result");
        const callback = () => {};
        const create = (options) => new IntersectionObserver(callback, options);
        const recordError = (name, options) => {
            try {
                create(options);
                result.setAttribute(`data-${name}`, "none");
            } catch (error) {
                result.setAttribute(`data-${name}`, error.name);
            }
        };
    "#
}

fn constructor_validation_success_cases() -> &'static str {
    r#"
        result.setAttribute("data-valid-margin", create({ rootMargin: "10px 5% -3px -2%" }).rootMargin);
        result.setAttribute("data-empty-margin", create({ rootMargin: "" }).rootMargin);
        result.setAttribute("data-space-margin", create({ rootMargin: "   " }).rootMargin);
        result.setAttribute("data-default-margin", create({}).rootMargin);
        result.setAttribute("data-sorted-thresholds", create({ threshold: [1, 0.25, 0] }).thresholds.join(","));
        result.setAttribute("data-empty-threshold", create({ threshold: [] }).thresholds.join(","));
        result.setAttribute("data-default-threshold", create({}).thresholds.join(","));
        result.setAttribute("data-zero-threshold", create({ threshold: 0 }).thresholds.join(","));
        result.setAttribute("data-one-threshold", create({ threshold: 1 }).thresholds.join(","));
        result.setAttribute("data-string-threshold", create({ threshold: "0.5" }).thresholds.join(","));
        result.setAttribute("data-set-thresholds", create({ threshold: new Set([0.5, 0]) }).thresholds.join(","));
        result.setAttribute("data-typed-array-thresholds", create({ threshold: new Float64Array([0.5, 0]) }).thresholds.join(","));
        let iteratorReads = 0;
        const iterable = {
            get [Symbol.iterator]() {
                iteratorReads += 1;
                return function*() { yield 0.5; };
            },
        };
        create({ threshold: iterable });
        result.setAttribute("data-iterator-reads", String(iteratorReads));
        result.setAttribute("data-null-root", String(create({ root: null }).root === null));
        result.setAttribute("data-document-root", String(create({ root: document }).root === document));
        result.setAttribute("data-element-root", String(create({ root: result }).root === result));
    "#
}

fn constructor_validation_error_cases() -> &'static str {
    r#"
        recordError("margin-unitless", { rootMargin: "10" });
        recordError("margin-em", { rootMargin: "10em" });
        recordError("margin-arity", { rootMargin: "1px 2px 3px 4px 5px" });
        recordError("margin-null", { rootMargin: null });
        recordError("threshold-low", { threshold: -0.25 });
        recordError("threshold-high", { threshold: 1.25 });
        recordError("threshold-nan", { threshold: 0 / 0 });
        recordError("threshold-infinity", { threshold: 1 / 0 });
        recordError("threshold-negative-infinity", { threshold: -1 / 0 });
        recordError("threshold-sparse", { threshold: [,] });
        recordError("threshold-boxed-bigint", { threshold: Object(1n) });
        recordError("threshold-iterator-error", {
            threshold: { [Symbol.iterator]() { throw new Error("iterator"); } },
        });
        recordError("threshold-noncallable-iterator", {
            threshold: { [Symbol.iterator]: 1, valueOf() { return 0.5; } },
        });
    "#
}

fn constructor_root_registration_cases() -> &'static str {
    r#"
        const add = Set.prototype.add;
        let rootRegistrations = 0;
        Set.prototype.add = function(value) {
            if (value && value.callback === callback) rootRegistrations += 1;
            return add.call(this, value);
        };
        recordError("root-window", { root: window });
        recordError("root-object", { root: {} });
        create({ root: result }).observe(result);
        Set.prototype.add = add;
        result.setAttribute("data-root-registrations", rootRegistrations);
    "#
}
