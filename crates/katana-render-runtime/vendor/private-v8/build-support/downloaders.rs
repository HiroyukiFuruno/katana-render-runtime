use std::env;
use std::fs;
use std::path::Path;
use which::which;

fn python() -> String {
  env::var("PYTHON").unwrap_or_else(|_| "python3".to_owned())
}

pub(super) fn download(url: &str, tmpfile: &Path) -> Result<(), String> {
  println!("Downloading {url}");
  let status =
    try_deno(url, tmpfile).or_else(|| try_python_or_curl(url, tmpfile));
  if status.is_some_and(|status| status.success()) {
    return Ok(());
  }
  if tmpfile.exists() {
    let _ = fs::remove_file(tmpfile);
  }
  Err("no downloader (deno, python, curl) could fetch the URL".to_string())
}

fn try_deno(url: &str, tmpfile: &Path) -> Option<std::process::ExitStatus> {
  let deno = which("deno").ok()?;
  println!("Trying with Deno...");
  crate::system::ProcessService::create_command(deno)
    .arg("eval")
    .arg(
      "const [url, path] = Deno.args; \
       const resp = await fetch(url); \
       if (!resp.ok) Deno.exit(1); \
       const file = await Deno.open(path, { write: true, create: true }); \
       await resp.body.pipeTo(file.writable);",
    )
    /* `deno eval` runs with implicit permissions and rejects `--allow-*`. */
    .arg("--")
    .arg(url)
    .arg(tmpfile)
    .status()
    .ok()
    .filter(std::process::ExitStatus::success)
}

fn try_python_or_curl(
  url: &str,
  tmpfile: &Path,
) -> Option<std::process::ExitStatus> {
  println!("Trying with Python...");
  let python_status = crate::system::ProcessService::create_command(python())
    .arg("./tools/download_file.py")
    .arg("--url")
    .arg(url)
    .arg("--filename")
    .arg(tmpfile)
    .status();
  match python_status {
    Ok(status) if status.success() => return Some(status),
    _ => {}
  }
  try_curl(url, tmpfile)
}

fn try_curl(url: &str, tmpfile: &Path) -> Option<std::process::ExitStatus> {
  println!("Python downloader failed, trying with curl.");
  crate::system::ProcessService::create_command("curl")
    .args(["-L", "-f", "-s", "-o"])
    .arg(tmpfile)
    .arg(url)
    .status()
    .ok()
}
