use super::BuildResult;
use std::env;

pub(super) fn print_link_flags() -> BuildResult<()> {
  println!("cargo:rustc-link-lib=static=rusty_v8");
  if should_dyn_link_libcxx() {
    print_cxx_stdlib_link_flag()?;
  }
  print_platform_link_flags()
}

fn should_dyn_link_libcxx() -> bool {
  env::var("CARGO_FEATURE_USE_CUSTOM_LIBCXX").is_err()
    || env::var("GN_ARGS").is_ok_and(|gn_args| {
      gn_args
        .split_whitespace()
        .any(|argument| argument == "use_custom_libcxx=false")
    })
}

fn print_cxx_stdlib_link_flag() -> BuildResult<()> {
  /* Based on https://github.com/alexcrichton/cc-rs/blob/fba7feded71ee4f63cfe885673ead6d7b4f2f454/src/lib.rs#L2462 */
  let Ok(stdlib) = env::var("CXXSTDLIB") else {
    print_default_cxx_stdlib_link_flag(&env::var("TARGET")?);
    return Ok(());
  };
  if !stdlib.is_empty() {
    println!("cargo:rustc-link-lib=dylib={stdlib}");
  }
  Ok(())
}

fn print_default_cxx_stdlib_link_flag(target: &str) {
  let library = if target.contains("msvc") {
    None
  } else if target.contains("apple")
    || target.contains("freebsd")
    || target.contains("openbsd")
  {
    Some("c++")
  } else if target.contains("android") {
    Some("c++_shared")
  } else {
    Some("stdc++")
  };
  if let Some(library) = library {
    println!("cargo:rustc-link-lib=dylib={library}");
  }
}

fn print_platform_link_flags() -> BuildResult<()> {
  let target_os = env::var("CARGO_CFG_TARGET_OS")?;
  let target_env = env::var("CARGO_CFG_TARGET_ENV")?;
  if target_os == "windows" {
    println!("cargo:rustc-link-lib=dylib=winmm");
    println!("cargo:rustc-link-lib=dylib=dbghelp");
  }
  if target_env == "msvc" {
    print_msvc_crt_link_flag();
  }
  Ok(())
}

fn print_msvc_crt_link_flag() {
  /* On Windows, including libcpmt[d]/msvcprt[d] explicitly links the C++ */
  /* standard library, which libc++ needs for exception_ptr internals. */
  let crt_static = env::var("CARGO_CFG_TARGET_FEATURE")
    .unwrap_or_default()
    .contains("crt-static");
  if crt_static {
    println!("cargo:rustc-link-lib=libcpmt");
  } else {
    println!("cargo:rustc-link-lib=dylib=msvcprt");
  }
}
