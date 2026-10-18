import { describe, expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  assertBinaryUnchanged,
  collectHtmlFiles,
  parseSuiteOptions,
  runProbe,
  scrollPositionsMatch,
} from "./fidelity-suite";
import { sha256File } from "./reference-capture";

describe("HTML fidelity suite options", () => {
  test("rejects mixing different probe builds in one capture suite", () => {
    expect(() => assertBinaryUnchanged("build-a", "build-a")).not.toThrow();
    expect(() => assertBinaryUnchanged("build-a", "build-b")).toThrow("binary changed");
  });
  test("requires a probe binary and accepts one fixture", () => {
    const options = parseSuiteOptions([
      "--binary",
      "target/debug/probe",
      "--input",
      "/tmp/fixture.html",
    ]);
    expect(options.binary).toContain("target/debug/probe");
    expect(options.input).toContain("/tmp/fixture.html");
  });

  test("rejects conflicting inputs and invalid thresholds", () => {
    expect(() =>
      parseSuiteOptions(["--binary", "probe", "--input", "one.html", "--inputs", "fixtures"]),
    ).toThrow();
    expect(() => parseSuiteOptions(["--binary", "probe", "--min-score", "101"])).toThrow();
  });

  test("requires matching scroll positions within the capture tolerance", () => {
    expect(scrollPositionsMatch(600, 600)).toBe(true);
    expect(scrollPositionsMatch(600, 600.009)).toBe(true);
    expect(scrollPositionsMatch(600, 600.011)).toBe(false);
  });

  test("rejects an HTML mutation during the KRR probe", () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "krr-fidelity-probe-"));
    const input = path.join(directory, "fixture.html");
    const binary = path.join(directory, "probe.sh");
    fs.writeFileSync(input, "<p>before</p>");
    const sourceSha = sha256File(input);
    fs.writeFileSync(
      binary,
      `#!/bin/sh
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--input" ]; then input="$2"; shift 2; else shift; fi
done
echo source_sha256=${sourceSha}
echo png=scroll-0.png scroll_y=0
echo png=scroll-600.png scroll_y=600
echo png=scroll-1800.png scroll_y=1800
echo png=scroll-1000000000.png scroll_y=6322
printf '<p>changed</p>' > "$input"
`,
    );
    fs.chmodSync(binary, 0o755);
    try {
      expect(() =>
        runProbe(
          {
            binary,
            input,
            inputs: directory,
            output: directory,
            minScore: 99,
          },
          input,
          path.join(directory, "krr"),
          { width: 1280, height: 900 },
        ),
      ).toThrow("HTML input changed");
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });

  test("rejects a probe executable replaced during its run", () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "krr-probe-replacement-"));
    const input = path.join(directory, "fixture.html");
    const binary = path.join(directory, "probe.sh");
    fs.writeFileSync(input, "<p>stable input</p>");
    fs.writeFileSync(binary, '#!/bin/sh\nprintf "\\n# changed\\n" >> "$0"\n');
    fs.chmodSync(binary, 0o755);
    try {
      expect(() =>
        runProbe(
          { binary, input, inputs: directory, output: directory, minScore: 99 },
          input,
          directory,
          { width: 620, height: 900 },
        ),
      ).toThrow("binary changed");
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });
});

test("discovers aspect fixtures alongside the complete document without temporary files", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "krr-aspect-fixtures-"));
  fs.mkdirSync(path.join(directory, "aspects"));
  fs.writeFileSync(path.join(directory, "document.html"), "<p>complete</p>");
  fs.writeFileSync(path.join(directory, "aspects", "tables.html"), "<table></table>");
  fs.writeFileSync(path.join(directory, ".private.html"), "<p>private</p>");
  fs.writeFileSync(path.join(directory, "aspects", "README.md"), "case descriptions");
  try {
    expect(
      collectHtmlFiles(directory)
        .map((file) => path.relative(directory, file))
        .sort(),
    ).toEqual([path.join("aspects", "tables.html"), "document.html"]);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
