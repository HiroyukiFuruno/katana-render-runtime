import { expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import {
  CLASSIC_CLI_ATTESTATION_SCOPE,
  LEGACY_CLI_ATTESTATION_SCOPE,
  verifyNativeClassicCapture,
} from "./native-classic-capture";
import {
  type CaptureFixture,
  program,
  replaceRequest,
  syncManifest,
  validate,
  withFixture,
  writeFile,
} from "./native-classic-capture-test-fixture";

test.each([false, true])("CLI strict capture accepts physical normal/Zen bytes: %s", (zen) => {
  withFixture((f) => {
    const result = verifyNativeClassicCapture(f.envelope, f.artifactDir, f.source);
    expect(result.capture.programs).toHaveLength(zen ? 5 : 4);
    expect(result.capture.acceptedConsumerClosure).toBe(false);
    expect(f.attestation.actualExit).toBeNull();
    expect(() => validate(f)).not.toThrow();
  }, zen);
});

test("CLI legacy capture absence retains the exact old scope", () => {
  withFixture((f) => {
    delete f.artifacts.classicCapture;
    f.attestation.scope = LEGACY_CLI_ATTESTATION_SCOPE;
    expect(() => validate(f)).not.toThrow();
    f.attestation.scope = CLASSIC_CLI_ATTESTATION_SCOPE;
    expect(() => validate(f)).toThrow();
  });
});

function fieldCases(
  fields: Record<string, unknown>,
  target: (f: CaptureFixture) => object = (f) => f.envelope.capture,
): Array<[string, (f: CaptureFixture) => void]> {
  return Object.entries(fields).map(([key, value]) => [
    key,
    (f) => {
      Object.assign(target(f), { [key]: value });
    },
  ]);
}

const cases: Array<[string, (f: CaptureFixture) => void, boolean?]> = [
  ...fieldCases({ extra: true }, (f) => f.envelope),
  ...fieldCases({ extra: true }),
  ...fieldCases({ extra: true }, (f) => f.envelope.capture.request),
  ...fieldCases({ extra: true }, (f) => program(f, 0)),
  ...fieldCases({
    schema: "unknown",
    status: "started",
    eventTarget: "other",
    scriptKind: "module",
    requiresExternalExitEvidence: false,
    acceptedConsumerClosure: true,
  }),
  [
    "missing program",
    (f) => {
      f.envelope.capture.programs.pop();
    },
  ],
  [
    "duplicate program",
    (f) => {
      f.envelope.capture.programs[1] = program(f, 0);
    },
  ],
  [
    "extra program",
    (f) => {
      f.envelope.capture.programs.push(program(f, 0));
    },
  ],
  ...fieldCases({ name: "mermaid.min.js", index: 1, completed: false }, (f) => program(f, 0)),
  ...fieldCases({ sha256: "0".repeat(64), size: -1 }, (f) => program(f, 0).file),
  [
    "wrong size",
    (f) => {
      program(f, 0).file.size += 1;
    },
  ],
  [
    "manifest hash",
    (f) => {
      f.envelope.manifest.sha256 = "0".repeat(64);
    },
    false,
  ],
  [
    "embedded/manifest mismatch",
    (f) => {
      f.envelope.capture.status = "started";
    },
    false,
  ],
  [
    "valid embedded/manifest mismatch",
    (f) => {
      program(f, 0).file.sha256 = "0".repeat(64);
    },
    false,
  ],
  [
    "manifest missing",
    (f) => {
      fs.renameSync(f.envelope.manifest.path, path.join(f.root, "moved"));
    },
    false,
  ],
  [
    "request missing field",
    (f) => {
      delete f.request.arrow;
      replaceRequest(f);
    },
  ],
  [
    "request extra field",
    (f) => {
      f.request.extra = "x";
      replaceRequest(f);
    },
  ],
  [
    "request non-string field",
    (f) => {
      f.request.theme = 4;
      replaceRequest(f);
    },
  ],
  [
    "request source mismatch",
    (f) => {
      f.request.source = "other";
      replaceRequest(f);
    },
  ],
  [
    "Zen request normal tuple",
    (f) => {
      f.request.diagramType = "zenuml";
      replaceRequest(f);
    },
  ],
  ...["reserialized", "other callee", "extra statement"].map<[string, (f: CaptureFixture) => void]>(
    (kind) => [
      kind,
      (f) => {
        const last = program(f, 3);
        const pretty = JSON.stringify(f.request, null, 2);
        const bytes =
          kind === "reserialized"
            ? `katanaRunMermaidRuntime(${JSON.stringify(f.request)});`
            : kind === "other callee"
              ? `other(${pretty});`
              : `katanaRunMermaidRuntime(${pretty});void 0;`;
        last.file = writeFile(last.file.path, bytes);
      },
    ],
  ),
  [
    "invalid UTF8 script",
    (f) => {
      const first = program(f, 0);
      first.file = writeFile(first.file.path, Buffer.from([0xff]));
    },
  ],
  [
    "invalid UTF8 request",
    (f) => {
      f.envelope.capture.request = writeFile(f.envelope.capture.request.path, Buffer.from([0xff]));
    },
  ],
  [
    "request BOM",
    (f) => {
      f.envelope.capture.request = writeFile(
        f.envelope.capture.request.path,
        `\ufeff${JSON.stringify(f.request)}`,
      );
    },
  ],
  [
    "path alias",
    (f) => {
      program(f, 1).file = program(f, 0).file;
    },
  ],
  [
    "traversal path",
    (f) => {
      program(f, 0).file.path =
        `${f.artifactDir}/../${path.basename(f.artifactDir)}/classic-script-00.js`;
    },
  ],
  [
    "different attempt",
    (f) => {
      program(f, 0).file = writeFile(path.join(f.root, "classic-script-00.js"), "var unit0 = 0;");
    },
  ],
  [
    "file symlink",
    (f) => {
      const first = program(f, 0);
      const moved = path.join(f.root, "moved");
      fs.renameSync(first.file.path, moved);
      fs.symlinkSync(moved, first.file.path);
    },
  ],
  [
    "hardlink role",
    (f) => {
      const first = program(f, 0);
      const second = program(f, 1);
      fs.renameSync(second.file.path, path.join(f.root, "moved"));
      fs.linkSync(first.file.path, second.file.path);
      second.file = { ...first.file, path: second.file.path };
    },
  ],
];

test.each(cases.map(([name, mutate, refresh]) => [name, mutate, refresh ?? true] as const))(
  "strict capture rejects %s after coherent manifest refresh",
  (_name, mutate, refresh) => {
    withFixture((f) => {
      mutate(f);
      if (refresh !== false) syncManifest(f);
      expect(() => validate(f)).toThrow();
    });
  },
);

test.each([
  [
    "null capture",
    (f: CaptureFixture) => {
      f.artifacts.classicCapture = null;
    },
  ],
  [
    "invalid capture with legacy scope",
    (f: CaptureFixture) => {
      f.artifacts.classicCapture = {};
      f.attestation.scope = LEGACY_CLI_ATTESTATION_SCOPE;
    },
  ],
  [
    "capture with legacy scope",
    (f: CaptureFixture) => {
      f.attestation.scope = LEGACY_CLI_ATTESTATION_SCOPE;
    },
  ],
  [
    "CLI actualExit zero",
    (f: CaptureFixture) => {
      f.attestation.actualExit = 0;
    },
  ],
  [
    "unknown scope",
    (f: CaptureFixture) => {
      f.attestation.scope = `${CLASSIC_CLI_ATTESTATION_SCOPE} `;
    },
  ],
])("CLI strict branch rejects %s without fallback", (_name, mutate) => {
  withFixture((f) => {
    mutate(f);
    expect(() => validate(f)).toThrow();
  });
});

test("strict reader rejects symlink in artifact directory ancestry", () => {
  withFixture((f) => {
    const moved = path.join(f.root, "moved-artifacts");
    fs.renameSync(f.artifactDir, moved);
    fs.symlinkSync(moved, f.artifactDir);
    expect(() => validate(f)).toThrow();
  });
});

test("Zen capture cannot be relabelled normal with a refreshed request", () => {
  withFixture((f) => {
    f.request.diagramType = "flowchart";
    replaceRequest(f);
    syncManifest(f);
    expect(() => validate(f)).toThrow();
  }, true);
});
