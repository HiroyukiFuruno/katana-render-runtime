use super::super::html_browser::{HtmlBrowserOrigin, HtmlBrowserSource};
use std::path::{Path, PathBuf};
use url::Url;

#[derive(Clone)]
pub(in crate::renderer::backends) struct HtmlSubresourcePolicy {
    origin: Url,
    local_root: Option<PathBuf>,
}

impl HtmlSubresourcePolicy {
    pub(in crate::renderer::backends) fn from_source(source: &HtmlBrowserSource) -> Self {
        let origin = source.origin.url().clone();
        let local_root = local_root(&origin);
        Self { origin, local_root }
    }

    pub(in crate::renderer::backends) fn resolve_subresource(
        &self,
        reference: &str,
    ) -> Result<Url, String> {
        let url = self.resolve(reference)?;
        self.allows_subresource(reference, &url)
            .then_some(url)
            .ok_or_else(|| format!("subresource is not allowed: {reference}"))
    }

    pub(in crate::renderer::backends) fn resolve_navigation(
        &self,
        reference: &str,
    ) -> Result<HtmlBrowserOrigin, String> {
        let url = self.resolve(reference)?;
        self.allows_navigation(reference, &url)
            .then(|| HtmlBrowserOrigin::from_validated_url(url))
            .ok_or_else(|| format!("navigation is not allowed: {reference}"))
    }

    pub(super) fn resolve_same_origin_request(&self, reference: &str) -> Result<Url, String> {
        let url = self.resolve(reference)?;
        self.allows_same_origin(reference, &url)
            .then_some(url)
            .ok_or_else(|| format!("dynamic request is not allowed: {reference}"))
    }

    pub(super) fn resolve_iframe(&self, reference: &str) -> Result<Url, String> {
        let url = self.resolve(reference)?;
        match self.origin.scheme() {
            "file" => self.resolve_local_iframe(reference, url),
            "http" | "https" if valid_network_url(&url) && url.origin() == self.origin.origin() => {
                Ok(url)
            }
            _ => Err(format!("iframe is not allowed: {reference}")),
        }
    }

    fn resolve_local_iframe(&self, reference: &str, url: Url) -> Result<Url, String> {
        if url.scheme() != "file" || !relative_file_reference(reference) {
            return Err(format!("local iframe is not allowed: {reference}"));
        }
        let path = Self::local_file_path(&url, reference)?
            .canonicalize()
            .map_err(|error| format!("local iframe could not be resolved: {reference}: {error}"))?;
        if !self
            .local_root
            .as_ref()
            .is_some_and(|root| path.starts_with(root))
        {
            return Err(format!("local iframe is not allowed: {reference}"));
        }
        if path.parent() != self.local_root.as_deref() {
            return Err(format!(
                "local iframe must be in the document directory: {reference}"
            ));
        }
        Ok(url)
    }

    fn resolve(&self, reference: &str) -> Result<Url, String> {
        let Ok(url) = Url::parse(reference) else {
            return self
                .origin
                .join(reference)
                .map_err(|error| format!("resource URL is invalid: {reference}: {error}"));
        };
        let scheme = url.scheme().to_string();
        matches!(scheme.as_str(), "data" | "file" | "http" | "https")
            .then_some(url)
            .ok_or_else(|| format!("unsupported resource scheme: {scheme}"))
    }

    fn allows_subresource(&self, reference: &str, url: &Url) -> bool {
        match url.scheme() {
            "data" => true,
            "file" => self.allows_same_origin(reference, url),
            "https" => valid_network_url(url),
            "http" => self.origin.scheme() != "https" && valid_network_url(url),
            _ => false,
        }
    }

    fn allows_navigation(&self, reference: &str, url: &Url) -> bool {
        if self.origin.scheme() == "file" {
            return relative_file_reference(reference)
                && (self.is_same_document_fragment(reference, url)
                    || self.allows_local_navigation(url));
        }
        url.scheme() != "data" && self.allows_same_origin(reference, url)
    }

    fn is_same_document_fragment(&self, reference: &str, url: &Url) -> bool {
        if !reference.starts_with('#') || url.fragment().is_none() {
            return false;
        }
        let mut current = self.origin.clone();
        let mut target = url.clone();
        current.set_fragment(None);
        target.set_fragment(None);
        current == target
    }

    fn allows_same_origin(&self, reference: &str, url: &Url) -> bool {
        if self.origin.scheme() == "file" {
            return relative_file_reference(reference) && self.allows_local_file(url);
        }
        matches!(url.scheme(), "http" | "https") && url.origin() == self.origin.origin()
    }

    fn allows_local_file(&self, url: &Url) -> bool {
        self.local_root.as_ref().is_some_and(|root| {
            url.to_file_path()
                .ok()
                .and_then(|path| path.canonicalize().ok())
                .is_some_and(|path| path.starts_with(root))
        })
    }

    fn local_file_path(url: &Url, reference: &str) -> Result<PathBuf, String> {
        if url
            .host_str()
            .is_some_and(|host| !host.is_empty() && host != "localhost")
        {
            return Err(format!("local iframe URL is invalid: {reference}"));
        }
        url.to_file_path()
            .map_err(|_| format!("local iframe URL is invalid: {reference}"))
    }

    pub(in crate::renderer::backends) fn allows_local_navigation(&self, url: &Url) -> bool {
        self.local_root.as_ref().is_some_and(|root| {
            let Ok(path) = url.to_file_path() else {
                return false;
            };
            path.ancestors()
                .find(|candidate| candidate.exists())
                .and_then(|ancestor| ancestor.canonicalize().ok())
                .is_some_and(|ancestor| ancestor.starts_with(root))
        })
    }
}

fn local_root(origin: &Url) -> Option<PathBuf> {
    origin
        .to_file_path()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .and_then(|path| path.canonicalize().ok())
}

fn relative_file_reference(reference: &str) -> bool {
    Url::parse(reference).is_err()
        && !Path::new(reference).is_absolute()
        && !reference.starts_with('/')
}

fn valid_network_url(url: &Url) -> bool {
    url.host_str().is_some() && url.username().is_empty() && url.password().is_none()
}

#[cfg(test)]
mod tests {
    use super::HtmlSubresourcePolicy;
    use crate::renderer::backends::HtmlBrowserSource;
    use url::Url;

    #[test]
    fn unsupported_scheme_is_not_an_allowed_subresource() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = HtmlBrowserSource::new("<p>ok</p>", "https://example.test/index.html")?;
        let unsupported = Url::parse("ftp://example.test/image.png")?;
        let policy = HtmlSubresourcePolicy::from_source(&source);

        assert!(!policy.allows_subresource("ftp://example.test/image.png", &unsupported));
        Ok(())
    }

    #[test]
    fn remote_file_url_is_not_a_local_iframe_path() -> Result<(), Box<dyn std::error::Error>> {
        let remote = Url::parse("file://example.test/frame.html")?;
        let non_file = Url::parse("data:text/plain,frame")?;

        assert!(HtmlSubresourcePolicy::local_file_path(&remote, "frame.html").is_err());
        assert!(HtmlSubresourcePolicy::local_file_path(&non_file, "frame.html").is_err());
        Ok(())
    }

    #[test]
    fn dynamic_requests_are_limited_to_the_document_origin()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = HtmlBrowserSource::new("<p>ok</p>", "https://example.test/app/index.html")?;
        let policy = HtmlSubresourcePolicy::from_source(&source);

        assert_eq!(
            policy
                .resolve_same_origin_request("../state.json")?
                .as_str(),
            "https://example.test/state.json"
        );
        assert!(
            policy
                .resolve_same_origin_request("https://other.test/state.json")
                .is_err()
        );
        assert!(
            policy
                .resolve_same_origin_request("http://example.test/state.json")
                .is_err()
        );
        Ok(())
    }
}
