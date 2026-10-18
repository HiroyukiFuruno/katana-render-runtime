export const MOON_QUALITY_INPUTS = [
  "**/*",
  "!/tmp/**",
  "!/.moon/cache/**",
  "!/target/**",
  "!/crates/katana-render-runtime/target/**",
  "!**/.ruff_cache/**",
  "!**/.mypy_cache/**",
  "!**/.pytest_cache/**",
  "!/tests/fixtures/quality-cache/.moon/cache/**",
  "!/tests/fixtures/quality-cache/.outputs/**",
] as const;
