use anyhow::{Context, ensure};
use serde_json::Value;
use std::thread::ThreadId;

const REQUEST_FIELDS: u8 = 7;
const SCRIPT_FIELDS: u8 = 25;
const NORMAL_SCRIPT_COUNT: usize = 4;
const ZENUML_SCRIPT_COUNT: usize = 5;
const ZENUML_RUNTIME_POSITION: usize = 3;

pub(super) struct CapturedScript {
    pub(super) name: String,
    pub(super) code: String,
    completed: bool,
}

pub(super) struct CaptureState {
    thread: ThreadId,
    pub(super) request: Option<String>,
    expected: usize,
    pub(super) scripts: Vec<CapturedScript>,
    error: Option<String>,
}

impl CaptureState {
    pub(super) fn new() -> Self {
        Self {
            thread: std::thread::current().id(),
            request: None,
            expected: 0,
            scripts: Vec::new(),
            error: None,
        }
    }

    pub(super) fn event(&mut self, fields: EventFields) {
        if self.error.is_some() {
            return;
        }
        if let Err(error) = self.apply(fields) {
            self.error = Some(error.to_string());
        }
    }

    fn apply(&mut self, fields: EventFields) -> anyhow::Result<()> {
        ensure!(
            self.thread == std::thread::current().id(),
            "classic event thread differs"
        );
        ensure!(fields.error.is_none(), "invalid classic event field");
        match fields.phase.as_deref() {
            Some("request") => self.request(fields),
            Some("script") => self.script(fields),
            Some("complete") => self.complete(fields),
            _ => anyhow::bail!("unknown classic event phase"),
        }
    }

    fn request(&mut self, fields: EventFields) -> anyhow::Result<()> {
        ensure!(
            fields.seen == REQUEST_FIELDS,
            "classic request fields differ"
        );
        ensure!(
            self.request.is_none() && self.scripts.is_empty(),
            "duplicate classic request"
        );
        let count = fields.count.context("classic script count missing")?;
        ensure!(matches!(count, 4 | 5), "classic script count differs");
        self.expected = usize::try_from(count)?;
        self.request = Some(fields.request.context("classic request JSON missing")?);
        Ok(())
    }

    fn script(&mut self, fields: EventFields) -> anyhow::Result<()> {
        ensure!(fields.seen == SCRIPT_FIELDS, "classic script fields differ");
        ensure!(self.request.is_some(), "classic script precedes request");
        ensure!(self.scripts.len() < self.expected, "extra classic script");
        ensure!(
            self.scripts.last().is_none_or(|script| script.completed),
            "classic script incomplete"
        );
        let name = fields.name.context("classic script name missing")?;
        ensure!(name == self.expected_name(), "classic script order differs");
        self.scripts.push(CapturedScript {
            name,
            code: fields.code.context("classic script code missing")?,
            completed: false,
        });
        Ok(())
    }

    fn expected_name(&self) -> &'static str {
        match self.scripts.len() {
            0 => "mermaid-runtime.min.js",
            1 => "mermaid.min.js",
            2 => "mermaid-zenuml.min.js",
            ZENUML_RUNTIME_POSITION if self.expected == ZENUML_SCRIPT_COUNT => {
                "zenuml-runtime.min.js"
            }
            _ => "render-mermaid.js",
        }
    }

    fn complete(&mut self, fields: EventFields) -> anyhow::Result<()> {
        ensure!(fields.seen == 1, "classic completion fields differ");
        let script = self
            .scripts
            .last_mut()
            .context("classic completion has no script")?;
        ensure!(!script.completed, "duplicate classic completion");
        script.completed = true;
        Ok(())
    }

    pub(super) fn validate(&self, source: &str) -> anyhow::Result<&str> {
        ensure!(
            self.error.is_none(),
            "classic capture rejected: {:?}",
            self.error
        );
        let request = self
            .request
            .as_deref()
            .context("classic request not observed (cache hit or missing trace)")?;
        self.validate_request(request, source)?;
        ensure!(
            self.scripts.len() == self.expected,
            "classic script missing"
        );
        ensure!(
            self.scripts.iter().all(|script| script.completed),
            "classic evaluation incomplete"
        );
        ensure!(
            self.scripts.last().is_some_and(|last| {
                last.code == format!("katanaRunMermaidRuntime({request});")
            }),
            "classic request program differs"
        );
        Ok(request)
    }

    fn validate_request(&self, request: &str, source: &str) -> anyhow::Result<()> {
        let value: Value = serde_json::from_str(request)?;
        Self::validate_request_fields(&value)?;
        ensure!(
            value["source"].as_str() == Some(source),
            "classic render source differs"
        );
        let count = if value["diagramType"] == "zenuml" {
            ZENUML_SCRIPT_COUNT
        } else {
            NORMAL_SCRIPT_COUNT
        };
        ensure!(self.expected == count, "classic diagram type/count differs");
        Ok(())
    }

    fn validate_request_fields(value: &Value) -> anyhow::Result<()> {
        let object = value
            .as_object()
            .context("classic request is not an object")?;
        ensure!(object.len() == 9, "classic request field count differs");
        for field in [
            "source",
            "svgId",
            "theme",
            "background",
            "fill",
            "text",
            "stroke",
            "arrow",
            "diagramType",
        ] {
            ensure!(
                value[field].is_string(),
                "classic request field type differs: {field}"
            );
        }
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct EventFields {
    pub(super) phase: Option<String>,
    pub(super) request: Option<String>,
    pub(super) count: Option<u64>,
    pub(super) name: Option<String>,
    pub(super) code: Option<String>,
    pub(super) seen: u8,
    pub(super) error: Option<String>,
}
