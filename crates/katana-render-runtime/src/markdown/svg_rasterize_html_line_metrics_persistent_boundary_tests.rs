use super::super::remember;
use super::{
    FAMILY, Metrics, Request, SIZE, TEXT, VALUES, WEIGHT, clear_persistent,
    evict_original_factory_proof, lookup, real_metrics, with_render_scoped_stamp_batch,
};

type MeasuredRequest = (Metrics, Request);

fn acquire_request() -> Result<MeasuredRequest, String> {
    let metrics = real_metrics().ok_or("real line metrics missing")?;
    let request =
        Request::new(TEXT, FAMILY, SIZE, WEIGHT, false).ok_or("real factory token missing")?;
    assert!(request.token.is_usable());
    assert_eq!(lookup(&request), Some(metrics));
    Ok((metrics, request))
}

fn assert_invalid_request_cannot_publish(request: &Request, metrics: Metrics) {
    assert!(!request.token.is_usable());
    assert!(lookup(request).is_none());
    remember(request, metrics);
    assert!(VALUES.with(|cache| cache.borrow().is_empty()));
}

#[test]
fn retained_persistent_request_cannot_publish_outside_validated_render() -> Result<(), String> {
    clear_persistent();
    let (metrics, request) = with_render_scoped_stamp_batch(acquire_request)?;
    clear_persistent();
    assert_invalid_request_cannot_publish(&request, metrics);
    with_render_scoped_stamp_batch(|| -> Result<(), String> {
        let (current, fresh) = acquire_request()?;
        assert_eq!(current, metrics);
        assert!(fresh.token.is_usable());
        Ok(())
    })?;
    clear_persistent();
    Ok(())
}

#[test]
fn evicted_persistent_request_cannot_publish_and_fresh_token_reuses_metrics() -> Result<(), String>
{
    clear_persistent();
    let (metrics, request) = with_render_scoped_stamp_batch(acquire_request)?;
    evict_original_factory_proof();
    clear_persistent();
    with_render_scoped_stamp_batch(|| -> Result<(), String> {
        assert_invalid_request_cannot_publish(&request, metrics);
        let (current, fresh) = acquire_request()?;
        assert_eq!(current, metrics);
        assert!(lookup(&request).is_none());
        assert_eq!(lookup(&fresh), Some(metrics));
        Ok(())
    })?;
    clear_persistent();
    Ok(())
}
