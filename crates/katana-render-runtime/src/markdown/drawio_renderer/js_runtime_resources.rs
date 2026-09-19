use base64::Engine;

#[path = "js_runtime_resource_archive.rs"]
mod archive;
#[path = "js_runtime_resource_selector.rs"]
mod selector;
use archive::DrawioResourceArchive;
use selector::DrawioResourceSelector;

pub(super) struct DrawioResourceCatalog;

impl DrawioResourceCatalog {
    pub(super) fn builtin(source: &str) -> Result<Vec<DrawioResource>, String> {
        let selector = DrawioResourceSelector::new(source);
        let mut resources = Vec::new();
        DrawioResourceArchive::collect(&selector, &mut resources)?;
        Ok(resources)
    }
}

pub(super) struct DrawioResource {
    pub(super) path: String,
    pub(super) mime_type: &'static str,
    pub(super) content: String,
    pub(super) encoding: DrawioResourceEncoding,
}

pub(super) enum DrawioResourceEncoding {
    Text,
    Base64,
}

impl DrawioResourceEncoding {
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Base64 => "base64",
        }
    }
}

fn drawio_resource(path: String, contents: &[u8]) -> DrawioResource {
    let encoding = encoding_for_path(&path);
    let mime_type = mime_type_for_path(&path);
    let content = resource_content(contents, &encoding);
    DrawioResource {
        path,
        mime_type,
        content,
        encoding,
    }
}

fn resource_content(contents: &[u8], encoding: &DrawioResourceEncoding) -> String {
    match encoding {
        DrawioResourceEncoding::Text => String::from_utf8_lossy(contents).into_owned(),
        DrawioResourceEncoding::Base64 => {
            base64::engine::general_purpose::STANDARD.encode(contents)
        }
    }
}

fn encoding_for_path(path: &str) -> DrawioResourceEncoding {
    if path.ends_with(".xml") || path.ends_with(".js") {
        return DrawioResourceEncoding::Text;
    }
    DrawioResourceEncoding::Base64
}

fn mime_type_for_path(path: &str) -> &'static str {
    if path.ends_with(".xml") {
        return "text/xml";
    }
    if path.ends_with(".js") {
        return "application/javascript";
    }
    if path.ends_with(".svg") {
        return "image/svg+xml";
    }
    if path.ends_with(".png") {
        return "image/png";
    }
    if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        return "image/jpeg";
    }
    if path.ends_with(".gif") {
        return "image/gif";
    }
    "application/octet-stream"
}

#[cfg(test)]
#[path = "js_runtime_resources_tests.rs"]
mod tests;
