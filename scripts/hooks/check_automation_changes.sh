#!/usr/bin/env bash
set -euo pipefail

uv run --no-project --with-requirements scripts/release/html-performance-requirements.lock \
  env PYTHONPATH=scripts/hooks:scripts/release:scripts/build:scripts/quality \
  python3 -m unittest \
  validation_scope_test draft_ci_contract_test html_performance_observer_workflow_test \
  local_html_performance_evidence_test local_macos_workflow_contract_test \
  native_coverage_reuse_test publish_crates_test release_workflow_contract_test \
  verify_ci_quality_evidence_test verify_completed_release_context_test
