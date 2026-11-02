"""Reviewed physical inputs of the runtime bundle definitions test."""
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess

import runtime_script_tool_fingerprint_inputs as common

PROFILE = "runtime-unused-bundle-definitions"
SEMANTIC_SCOPE = "krr-runtime-definitions-profile-inputs-v1"
PROBE = "scripts/runtime-bundles/runtime-definitions-input-probe.ts"
REVIEWED = {
    "scripts/runtime-bundles/runtime-bundle-definitions_test.ts": "fa4c7d9bfb4985ae06bbc5460c38242a985b5fc3e5daadb08f7c1ee0d598f304",
    "scripts/runtime-bundles/runtime-bundle-definitions.ts": "f6fb89d05da838b8344e819d4a2b3e1516d52f4f9770e5325c59c0cdbcc2973e",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mathjax/runtime_definition.ts": "cf14afd754ec08c404949f0198e5cb2ab9a53a1ad9b760af9f2326b351bcad73",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/zenuml/runtime_definition.ts": "b401e9feb6bf87d2201fcfea980cec882149a7f973146f6e627521301bbead77",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mermaid/runtime_definition.ts": "227089b292f91475f21093c16eacc1fef532af751afe6eb01d7634e40b220ca2",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/drawio/runtime_definition.ts": "24f2f5707c70030a1410dad4c4a90ce4d3fe9a267235b0b5dd46fa396620a3f6",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/mermaid/runtime_entrypoint.ts": "8456e1d975a1c3eef4dd3ab542e1c2802b289f3d87b20715ee7d3cc9a4d97296",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/shared/runtime_bundle.ts": "bb86941face8d5cab3d9d7c30aaaf5559527b45c69b720f820544588bc3c0a2f",
    "crates/katana-render-runtime/src/markdown/diagram_runtime/source/shared/dom_fragments.ts": "8eea2f6979ca58c88bac6d56854468c06ff2822c199f53376091b0653bb86564",
}
COLLECTOR = {
    "scripts/runtime-bundles/runtime-unused-import-scan.ts": "c63b737b53103dddf77fa404da6db711573f26e380677d6e18bb7fc4a3b53a61",
    PROBE: "a6501a251f877d0bcd378bcf9ea8f9da3aaf7cb284a8fd36dfc0d950b49f80dd",
}


def _records(repo: Path) -> list[dict[str, object]]:
    expected = {**REVIEWED, **COLLECTOR}
    rows = [common._regular_record(repo, name) for name in sorted(expected)]
    if any(row.get("sha256") != expected[name] for name, row in zip(sorted(expected), rows, strict=True)):
        raise ValueError("definitions_reviewed_source_changed")
    return rows


def collect(repo: Path) -> str:
    """Refuse new imports, dynamic behavior, or modified reviewed inputs."""
    before = _records(repo)
    argv = ["bun", PROBE]
    expected_sources = sorted(str(repo / name) for name in REVIEWED)
    expected = {"sources": expected_sources, "tools": [], "dedicatedTools": []}
    for _ in range(2):
        run = subprocess.run(argv, cwd=repo, env=dict(os.environ), capture_output=True, check=False)
        if run.returncode != 0 or json.loads(run.stdout) != expected or _records(repo) != before:
            raise ValueError("definitions_import_inventory_changed")
    return common._digest({"scope": SEMANTIC_SCOPE, "records": before, "imports": expected})
