#[path = "tests_policy.rs"]
mod policy;
#[path = "tests_transport.rs"]
mod transport;

mod iframe;
mod network_iframe;
#[path = "test_support.rs"]
pub(in crate::renderer::backends::html_subresources) mod support;
