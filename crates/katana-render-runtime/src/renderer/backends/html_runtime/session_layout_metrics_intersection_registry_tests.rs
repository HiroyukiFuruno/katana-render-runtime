use super::super::StaticHtmlRuntime;
use super::super::types::HtmlRuntimeError;
use super::StaticHtmlRuntimeSession;

#[test]
fn intersection_observer_registry_contains_only_active_observers() -> Result<(), HtmlRuntimeError> {
    let session = start(observer_registry_lifecycle_source());
    let snapshot = session.snapshot()?;

    assert!(snapshot.contains(r#"data-registrations="2""#));
    assert!(snapshot.contains(r#"data-removals="2""#));
    Ok(())
}

fn start(source: &str) -> StaticHtmlRuntimeSession {
    let start = StaticHtmlRuntime.start(source);
    assert!(start.is_ok());
    let mut sessions = start.into_iter().collect::<Vec<_>>();
    sessions.remove(0)
}

fn observer_registry_lifecycle_source() -> &'static str {
    r#"<p id="target">target</p><p id="result"></p><script>
        const result = document.getElementById("result");
        const target = document.getElementById("target");
        const callback = () => {};
        const add = Set.prototype.add;
        const remove = Set.prototype.delete;
        let registrations = 0;
        let removals = 0;
        Set.prototype.add = function(value) {
            if (value && value.callback === callback) registrations += 1;
            return add.call(this, value);
        };
        Set.prototype.delete = function(value) {
            if (value && value.callback === callback) removals += 1;
            return remove.call(this, value);
        };
        const observer = new IntersectionObserver(callback);
        observer.observe(target);
        observer.unobserve(target);
        observer.observe(target);
        observer.unobserve(target);
        Set.prototype.add = add;
        Set.prototype.delete = remove;
        result.setAttribute("data-registrations", registrations);
        result.setAttribute("data-removals", removals);
    </script>"#
}
