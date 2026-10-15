use std::{fs, path::PathBuf};

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
pub(super) struct OperationPlan {
    pub(super) fixture_sha256: String,
    pub(super) operation: String,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) expected_dom_state_path: PathBuf,
    pub(super) expected_dom_state_sha256: String,
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
    let operation_plan: OperationPlan = serde_json::from_slice(&fs::read(&operation_plan_path)?)?;
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
    use super::worker_sample;
    use std::{ffi::OsString, iter};

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
