#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlRuntimeError {
    ExternalScript(String),
    Subresource(String),
    JavaScriptCompile(String),
    JavaScriptException(String),
    DomBridge(String),
    ExecutionTimeout,
}

impl std::fmt::Display for HtmlRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExternalScript(source) => {
                write!(formatter, "external script is not supported: {source}")
            }
            Self::Subresource(message) => write!(formatter, "HTML subresource error: {message}"),
            Self::JavaScriptCompile(message) => {
                write!(formatter, "JavaScript compile error: {message}")
            }
            Self::JavaScriptException(message) => {
                write!(formatter, "JavaScript exception: {message}")
            }
            Self::DomBridge(message) => write!(formatter, "HTML DOM bridge error: {message}"),
            Self::ExecutionTimeout => write!(formatter, "JavaScript execution timed out"),
        }
    }
}

impl std::error::Error for HtmlRuntimeError {}

#[cfg(test)]
mod tests {
    use super::HtmlRuntimeError;

    #[test]
    fn subresource_error_preserves_diagnostic_context() {
        assert_eq!(
            HtmlRuntimeError::Subresource("blocked style.css".to_string()).to_string(),
            "HTML subresource error: blocked style.css"
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HtmlNodeId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum HtmlRuntimeEventKind {
    Blur,
    Change,
    Click,
    Focus,
    Input,
    KeyDown,
    KeyUp,
    Scroll,
    Toggle,
}

impl HtmlRuntimeEventKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Blur => "blur",
            Self::Change => "change",
            Self::Click => "click",
            Self::Focus => "focus",
            Self::Input => "input",
            Self::KeyDown => "keydown",
            Self::KeyUp => "keyup",
            Self::Scroll => "scroll",
            Self::Toggle => "toggle",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlNavigationIntent {
    pub href: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlRuntimeEvent {
    Blur { target: HtmlNodeId },
    Change { target: HtmlNodeId },
    Click { target: HtmlNodeId },
    Focus { target: HtmlNodeId },
    Input { target: HtmlNodeId },
    KeyDown { target: HtmlNodeId, key: String },
    KeyUp { target: HtmlNodeId, key: String },
    Scroll { target: HtmlNodeId },
    Toggle { target: HtmlNodeId },
}

impl HtmlRuntimeEvent {
    pub(crate) fn kind(&self) -> HtmlRuntimeEventKind {
        match self {
            Self::Blur { .. } => HtmlRuntimeEventKind::Blur,
            Self::Change { .. } => HtmlRuntimeEventKind::Change,
            Self::Click { .. } => HtmlRuntimeEventKind::Click,
            Self::Focus { .. } => HtmlRuntimeEventKind::Focus,
            Self::Input { .. } => HtmlRuntimeEventKind::Input,
            Self::KeyDown { .. } => HtmlRuntimeEventKind::KeyDown,
            Self::KeyUp { .. } => HtmlRuntimeEventKind::KeyUp,
            Self::Scroll { .. } => HtmlRuntimeEventKind::Scroll,
            Self::Toggle { .. } => HtmlRuntimeEventKind::Toggle,
        }
    }

    pub(crate) fn target(&self) -> HtmlNodeId {
        match self {
            Self::Blur { target }
            | Self::Change { target }
            | Self::Click { target }
            | Self::Focus { target }
            | Self::Input { target }
            | Self::KeyDown { target, .. }
            | Self::KeyUp { target, .. }
            | Self::Scroll { target }
            | Self::Toggle { target } => *target,
        }
    }

    pub(crate) fn key(&self) -> Option<&str> {
        match self {
            Self::KeyDown { key, .. } | Self::KeyUp { key, .. } => Some(key),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlRuntimeDispatch {
    pub content: String,
    pub navigation: Option<HtmlNavigationIntent>,
}

pub(super) enum DomValue {
    Undefined,
    Null,
    String(String),
    NodeId(u64),
    NodeIds(Vec<u64>),
}
