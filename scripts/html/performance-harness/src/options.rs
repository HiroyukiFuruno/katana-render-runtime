use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(feature = "baseline")]
use baseline::HtmlBrowserViewport;
#[cfg(feature = "candidate")]
use candidate::HtmlBrowserViewport;

#[derive(Debug)]
pub(super) struct Options {
    pub(super) input: PathBuf,
    pub(super) output: PathBuf,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) scale: f32,
    pub(super) operation_plan: OperationPlan,
    pub(super) operation_plan_path: PathBuf,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum OperationPlan {
    Legacy(LegacyOperationPlan),
    Catalog(CatalogOperationPlan),
}

#[derive(Debug, serde::Deserialize)]
pub(super) struct LegacyOperationPlan {
    pub(super) fixture_sha256: String,
    pub(super) operation: String,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) expected_dom_state_path: PathBuf,
    pub(super) expected_dom_state_sha256: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CatalogOperationPlan {
    pub(super) profile: String,
    pub(super) condition_id: String,
    pub(super) case_id: String,
    pub(super) state: String,
    pub(super) operation_kind: String,
    pub(super) target_selector: Option<String>,
    pub(super) fixture_sha256: String,
    pub(super) viewport: CatalogViewport,
    pub(super) operation_evidence_path: PathBuf,
    pub(super) operation_evidence_sha256: String,
    pub(super) evidence_binary_sha256: String,
    pub(super) operation_stream: Vec<CatalogInput>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CatalogViewport {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) scale: f32,
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum CatalogInput {
    PointerMove { x: f32, y: f32 },
    PointerDown { x: f32, y: f32, button: u8 },
    PointerUp { x: f32, y: f32, button: u8 },
    Text { text: String },
    KeyDown { key: String },
    KeyUp { key: String },
    Scroll { delta_x: f32, delta_y: f32 },
    AdvanceTime { elapsed_ms: f64 },
}

impl OperationPlan {
    pub(super) fn fixture_sha256(&self) -> &str {
        match self {
            Self::Legacy(plan) => &plan.fixture_sha256,
            Self::Catalog(plan) => &plan.fixture_sha256,
        }
    }

    pub(super) fn operation(&self) -> &str {
        match self {
            Self::Legacy(plan) => &plan.operation,
            Self::Catalog(plan) => &plan.operation_kind,
        }
    }

    pub(super) fn expected_dom_state_sha256(&self) -> &str {
        match self {
            Self::Legacy(plan) => &plan.expected_dom_state_sha256,
            Self::Catalog(_) => "",
        }
    }
}

pub(super) fn worker_sample<I>(
    args: &mut std::iter::Peekable<I>,
) -> Result<Option<usize>, Box<dyn std::error::Error>>
where
    I: Iterator<Item = std::ffi::OsString>,
{
    if !args
        .peek()
        .map(|value| value == "--worker")
        .unwrap_or(false)
    {
        return Ok(None);
    }
    args.next();
    let sample = args
        .next()
        .ok_or("worker sample is required")?
        .to_string_lossy()
        .parse()?;
    Ok(Some(sample))
}

pub(super) fn parse_options(
    mut args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<Options, Box<dyn std::error::Error>> {
    let input = required_path(&mut args, "input HTML is required")?;
    let output = required_path(&mut args, "output JSON is required")?;
    let operation_plan_path = required_path(&mut args, "operation plan is required")?;
    let width = required_number(&mut args, "viewport width is required")?;
    let height = required_number(&mut args, "viewport height is required")?;
    let scale = required_number(&mut args, "viewport scale is required")?;
    if args.next().is_some() {
        return Err("unexpected trailing argument".into());
    }
    HtmlBrowserViewport::new(width, height, scale)?;
    let operation_plan = parse_operation_plan(&operation_plan_path, width, height, scale)?;
    Ok(Options {
        input,
        output,
        width,
        height,
        scale,
        operation_plan,
        operation_plan_path,
    })
}

fn parse_operation_plan(
    path: &Path,
    width: u32,
    height: u32,
    scale: f32,
) -> Result<OperationPlan, Box<dyn std::error::Error>> {
    let operation_plan: OperationPlan = serde_json::from_slice(&fs::read(path)?)?;
    if let OperationPlan::Catalog(plan) = &operation_plan {
        if plan.profile != "catalog-plan84"
            || plan.condition_id.is_empty()
            || plan.case_id.is_empty()
            || plan.state.is_empty()
            || plan.operation_kind.is_empty()
            || plan.viewport.width != width
            || plan.viewport.height != height
            || plan.viewport.scale != scale
        {
            return Err("catalog operation plan does not match the worker viewport".into());
        }
    }
    Ok(operation_plan)
}

fn required_path(
    args: &mut impl Iterator<Item = std::ffi::OsString>,
    error: &'static str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(args.next().ok_or(error)?))
}

fn required_number<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = std::ffi::OsString>,
    error: &'static str,
) -> Result<T, Box<dyn std::error::Error>>
where
    T::Err: std::error::Error + 'static,
{
    Ok(args.next().ok_or(error)?.to_string_lossy().parse()?)
}

#[cfg(test)]
mod tests {
    use super::{parse_options, worker_sample};
    use super::{LegacyOperationPlan, OperationPlan};
    use serde_json::json;
    use std::{ffi::OsString, fs, iter, path::PathBuf};

    fn parse_catalog_plan(
        plan: serde_json::Value,
        trailing: bool,
    ) -> Result<super::Options, Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let plan_path = directory.path().join("plan.json");
        fs::write(&plan_path, serde_json::to_vec(&plan)?)?;
        let mut args = vec![
            OsString::from("input.html"),
            OsString::from("result.json"),
            plan_path.into_os_string(),
            OsString::from("1280"),
            OsString::from("900"),
            OsString::from("1"),
        ];
        if trailing {
            args.push(OsString::from("unexpected"));
        }
        parse_options(args.into_iter())
    }

    fn legacy_plan() -> OperationPlan {
        OperationPlan::Legacy(LegacyOperationPlan {
            fixture_sha256: "legacy".into(),
            operation: "click".into(),
            x: 1.0,
            y: 2.0,
            expected_dom_state_path: PathBuf::from("state.json"),
            expected_dom_state_sha256: "state".into(),
        })
    }

    fn catalog_model() -> Result<OperationPlan, Box<dyn std::error::Error>> {
        Ok(OperationPlan::Catalog(serde_json::from_value(
            crate::test_support::TestSupport::catalog_test_plan(),
        )?))
    }

    #[test]
    fn operation_plan_accessors_preserve_legacy_and_catalog_contracts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let legacy = legacy_plan();
        let catalog = catalog_model()?;
        assert_eq!(legacy.fixture_sha256(), "legacy");
        assert_eq!(legacy.operation(), "click");
        assert_eq!(legacy.expected_dom_state_sha256(), "state");
        assert_eq!(catalog.fixture_sha256(), "a".repeat(64));
        assert_eq!(catalog.operation(), "script-await-timer-microtask");
        assert_eq!(catalog.expected_dom_state_sha256(), "");
        Ok(())
    }

    #[test]
    fn parse_options_accepts_catalog_and_rejects_bounded_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let parsed =
            parse_catalog_plan(crate::test_support::TestSupport::catalog_test_plan(), false)?;
        assert_eq!(
            parsed.operation_plan.operation(),
            "script-await-timer-microtask"
        );
        let trailing = parse_error(crate::test_support::TestSupport::catalog_test_plan(), true)?;
        assert_eq!(trailing, "unexpected trailing argument");
        for (label, field, value) in [
            ("profile", "profile", json!("wrong-profile")),
            ("identity", "conditionId", json!("")),
            (
                "viewport",
                "viewport",
                json!({"width": 640, "height": 900, "scale": 1.0}),
            ),
        ] {
            let mut plan = crate::test_support::TestSupport::catalog_test_plan();
            plan[field] = value;
            let error = parse_error(plan, false)?;
            assert_eq!(
                error, "catalog operation plan does not match the worker viewport",
                "{label}"
            );
        }
        Ok(())
    }

    fn parse_error(
        plan: serde_json::Value,
        trailing: bool,
    ) -> Result<String, Box<dyn std::error::Error>> {
        match parse_catalog_plan(plan, trailing) {
            Ok(_) => Err("invalid catalog plan was accepted".into()),
            Err(error) => Ok(error.to_string()),
        }
    }

    #[test]
    fn normal_invocation_preserves_first_input_argument() -> Result<(), Box<dyn std::error::Error>>
    {
        let input = OsString::from("fixture.html");
        let mut args = vec![
            input.clone(),
            OsString::from("result.json"),
            OsString::from("plan.json"),
        ]
        .into_iter()
        .peekable();
        assert_eq!(worker_sample(&mut args)?, None);
        assert_eq!(args.next(), Some(input));
        Ok(())
    }

    #[test]
    fn worker_invocation_consumes_only_marker_and_sample() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut args = vec![
            OsString::from("--worker"),
            OsString::from("4"),
            OsString::from("fixture.html"),
        ]
        .into_iter()
        .peekable();
        assert_eq!(worker_sample(&mut args)?, Some(4));
        assert_eq!(args.next(), Some(OsString::from("fixture.html")));
        Ok(())
    }

    #[test]
    fn worker_marker_requires_a_sample() {
        let mut args = iter::once(OsString::from("--worker")).peekable();
        assert!(worker_sample(&mut args).is_err());
    }
}
