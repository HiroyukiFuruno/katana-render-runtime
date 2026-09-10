use super::*;
use std::sync::{Arc, Barrier};
use std::thread;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
type OpenWorker = thread::JoinHandle<Result<(), String>>;
const TEST_VIEWPORT_WIDTH: u32 = 160;
const TEST_VIEWPORT_HEIGHT: u32 = 120;

#[test]
fn open_starts_the_in_process_rust_runtime() -> TestResult {
    let mut session = HtmlRuntime.open(test_source()?, viewport()?)?;

    assert!(session.has_in_process_runtime());
    assert_eq!(
        session.latest_frame().map(|frame| frame.generation),
        Some(1)
    );
    session.close()?;
    Ok(())
}

#[test]
fn concurrent_openers_each_receive_an_initial_frame() -> TestResult {
    const CALLERS: usize = 4;
    join_concurrent_open_workers(start_concurrent_open_workers(CALLERS))
}

#[test]
fn open_reports_a_poisoned_cold_start_lock() -> TestResult {
    COLD_OPEN_COMPLETED.store(false, std::sync::atomic::Ordering::Release);
    let lock = COLD_OPEN_LOCK.get_or_init(|| std::sync::Mutex::new(()));
    let poisoner = thread::spawn(|| {
        let mutex = match COLD_OPEN_LOCK.get() {
            Some(mutex) => mutex,
            None => std::panic::resume_unwind(Box::new("cold-start lock should be initialized")),
        };
        let _guard = match mutex.lock() {
            Ok(guard) => guard,
            Err(_) => std::panic::resume_unwind(Box::new(
                "cold-start lock should not already be poisoned",
            )),
        };
        std::panic::resume_unwind(Box::new("poison the cold-start lock for the error path"));
    });
    assert!(poisoner.join().is_err());

    let error = match HtmlRuntime.open(test_source()?, viewport()?) {
        Ok(_) => return Err("a poisoned cold-start lock must fail closed".into()),
        Err(error) => error,
    };
    assert!(matches!(error, HtmlBrowserError::RuntimeFailure { .. }));
    lock.clear_poison();
    COLD_OPEN_COMPLETED.store(true, std::sync::atomic::Ordering::Release);
    Ok(())
}

fn start_concurrent_open_workers(callers: usize) -> Vec<OpenWorker> {
    let start = Arc::new(Barrier::new(callers));
    (0..callers)
        .map(|_| {
            let start = Arc::clone(&start);
            thread::spawn(move || concurrent_open_worker(start))
        })
        .collect()
}

fn join_concurrent_open_workers(workers: Vec<OpenWorker>) -> TestResult {
    for worker in workers {
        worker
            .join()
            .map_err(|_| "concurrent HtmlRuntime::open worker panicked")??;
    }
    Ok(())
}

fn concurrent_open_worker(start: Arc<Barrier>) -> Result<(), String> {
    start.wait();
    let source = HtmlBrowserSource::new("<p>cold start</p>", "https://example.test/index.html")
        .map_err(|error| error.to_string())?;
    let viewport = HtmlBrowserViewport::new(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT, 1.0)
        .map_err(|error| error.to_string())?;
    let mut session = HtmlRuntime
        .open(source, viewport)
        .map_err(|error| error.to_string())?;
    let initial_generation = session.latest_frame().map(|frame| frame.generation);
    session.close().map_err(|error| error.to_string())?;
    (initial_generation == Some(1))
        .then_some(())
        .ok_or_else(|| "missing initial frame".to_string())
}

#[test]
fn html_runtime_traits_are_value_like() {
    let runtime = HtmlRuntime;
    let copied = runtime;
    let cloned = <HtmlRuntime as Clone>::clone(&copied);

    assert_eq!(format!("{runtime:?}"), "HtmlRuntime");
    assert_eq!(format!("{cloned:?}"), format!("{copied:?}"));
}

#[test]
fn test_source_helper_propagates_invalid_origin() {
    assert!(matches!(
        test_source_with_origin("not a url"),
        Err(error)
            if error
                .downcast_ref::<HtmlBrowserError>()
                .is_some_and(|error| matches!(error, HtmlBrowserError::InvalidOrigin { .. }))
    ));
}

fn test_source() -> TestResult<HtmlBrowserSource> {
    test_source_with_origin("https://example.test/index.html")
}

fn test_source_with_origin(origin: &str) -> TestResult<HtmlBrowserSource> {
    Ok(HtmlBrowserSource::new("<p>ok</p>", origin)?)
}

fn viewport() -> TestResult<HtmlBrowserViewport> {
    Ok(HtmlBrowserViewport::new(
        TEST_VIEWPORT_WIDTH,
        TEST_VIEWPORT_HEIGHT,
        1.0,
    )?)
}
