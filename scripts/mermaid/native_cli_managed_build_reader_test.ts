import { expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import {
  assertCurrentGitSourceSet,
  digestStringRecord,
  type FileRecord,
  validateCapacityReport,
  verifyNativeBuildReceipt,
} from "./native-build-receipt";
import { writeFile } from "./native-classic-capture-test-fixture";
import { duplicateManagedFixtureJson } from "./native-cli-managed-build-input-test-fixture";
import { createManagedReaderFixture } from "./native-cli-managed-build-test-fixture";
import { verifyManagedCapacity } from "./native-cli-managed-cargo";
import { managedControlNames, parseManagedControlManifest } from "./native-cli-managed-inputs";
import { parseManagedJson } from "./native-cli-managed-json";
import { read } from "./native-cli-managed-process";

type Fixture = ReturnType<typeof createManagedReaderFixture>;
const controlManifest = {
  schema: "krr-native-cli-managed-control-inputs-v1",
  control: [...managedControlNames],
};
for (const [name, value] of Object.entries({
  unknown: { ...controlManifest, unexpected: true },
  schema: { ...controlManifest, schema: "other" },
  empty: { ...controlManifest, control: [] },
  duplicate: {
    ...controlManifest,
    control: [...controlManifest.control.slice(0, -1), controlManifest.control[0]],
  },
  absolute: { ...controlManifest, control: ["/tmp/escape", ...controlManifest.control.slice(1)] },
  traversal: {
    ...controlManifest,
    control: ["scripts/../escape", ...controlManifest.control.slice(1)],
  },
  "windows drive": {
    ...controlManifest,
    control: ["C:/escape", ...controlManifest.control.slice(1)],
  },
  backslash: {
    ...controlManifest,
    control: ["scripts\\escape", ...controlManifest.control.slice(1)],
  },
  "missing self": {
    ...controlManifest,
    control: [
      ...controlManifest.control.filter(
        (name) => name !== "scripts/mermaid/native-cli-managed-control-inputs.json",
      ),
      "scripts/mermaid/other.json",
    ],
  },
}))
  test(`managed control manifest rejects ${name}`, () => {
    expect(() => parseManagedControlManifest(JSON.stringify(value))).toThrow();
  });
test("managed control manifest rejects duplicate JSON keys", () => {
  expect(() =>
    parseManagedControlManifest(
      '{"schema":"krr-native-cli-managed-control-inputs-v1","schema":"other"}',
    ),
  ).toThrow();
});

function verify(fixture: Fixture) {
  writeFile(fixture.receiptPath, JSON.stringify(fixture.receipt));
  return verifyNativeBuildReceipt(
    fixture.receiptPath,
    fixture.binaryPath,
    fixture.runtimePath,
    fixture.root,
  );
}
function observation(fixture: Fixture) {
  fixture.receipt.observation = writeFile(
    fixture.receipt.observation.path,
    JSON.stringify(fixture.observation),
  );
}
function updateCapacityPreflight(
  fixture: Fixture,
  update: (value: { [key: string]: unknown }) => void,
) {
  const pin = fixture.receipt.capacityPreflight;
  const value = JSON.parse(fs.readFileSync(pin.path, "utf8")) as { [key: string]: unknown };
  update(value);
  fixture.receipt.capacityPreflight = writeFile(pin.path, JSON.stringify(value));
}
function updatePreflightCommand(fixture: Fixture, update: (argv: string[]) => string[]) {
  const pin = fixture.receipt.capacityPreflight;
  const evidence = JSON.parse(fs.readFileSync(pin.path, "utf8")) as { [key: string]: unknown };
  const argv = update(z.array(z.string()).parse(evidence.argv));
  const lifecyclePin = z.object({ path: z.string() }).passthrough().parse(evidence.lifecycle);
  const lifecycle = JSON.parse(fs.readFileSync(lifecyclePin.path, "utf8")) as {
    [key: string]: unknown;
  };
  evidence.argv = argv;
  lifecycle.argv = argv;
  evidence.lifecycle = writeFile(lifecyclePin.path, JSON.stringify(lifecycle));
  fixture.receipt.capacityPreflight = writeFile(pin.path, JSON.stringify(evidence));
}
function updatePreflightClosure(
  fixture: Fixture,
  update: (closure: Record<string, unknown>, ownerPid: number) => void,
) {
  const pin = fixture.receipt.capacityPreflight;
  const evidence = JSON.parse(fs.readFileSync(pin.path, "utf8")) as Record<string, unknown>;
  const lifecyclePin = z.object({ path: z.string() }).passthrough().parse(evidence.lifecycle);
  const lifecycle = JSON.parse(fs.readFileSync(lifecyclePin.path, "utf8")) as Record<
    string,
    unknown
  >;
  const closure = z.record(z.string(), z.unknown()).parse(lifecycle.nativeRawFdClosure);
  const ownerIdentity = z.record(z.string(), z.unknown()).parse(lifecycle.identity);
  update(closure, z.number().parse(ownerIdentity.pid));
  lifecycle.nativeRawFdClosure = closure;
  evidence.lifecycle = writeFile(lifecyclePin.path, JSON.stringify(lifecycle));
  fixture.receipt.capacityPreflight = writeFile(pin.path, JSON.stringify(evidence));
}
function updateCapacityEvidenceReport(
  evidencePin: FileRecord,
  update: (report: Record<string, unknown>) => void,
) {
  const evidence = JSON.parse(fs.readFileSync(evidencePin.path, "utf8")) as Record<string, unknown>;
  const stdout = z.object({ path: z.string() }).passthrough().parse(evidence.stdout);
  const report = z.record(z.string(), z.unknown()).parse(evidence.report);
  update(report);
  evidence.stdout = writeFile(stdout.path, JSON.stringify(report));
  const lifecyclePin = z.object({ path: z.string() }).passthrough().parse(evidence.lifecycle);
  const lifecycle = JSON.parse(fs.readFileSync(lifecyclePin.path, "utf8")) as Record<
    string,
    unknown
  >;
  const raw = z.array(z.record(z.string(), z.unknown())).parse(lifecycle.raw);
  const firstRaw = raw[0];
  if (firstRaw === undefined) throw new Error("preflight raw stdout missing");
  firstRaw.sha256 = z.object({ sha256: z.string() }).parse(evidence.stdout).sha256;
  lifecycle.raw = raw;
  evidence.lifecycle = writeFile(lifecyclePin.path, JSON.stringify(lifecycle));
  evidence.report = report;
  return writeFile(evidencePin.path, JSON.stringify(evidence));
}
function updateAcceptedPreflightReport(
  fixture: Fixture,
  update: (report: Record<string, unknown>) => void,
) {
  fixture.receipt.capacityPreflight = updateCapacityEvidenceReport(
    fixture.receipt.capacityPreflight,
    update,
  );
}
function updateCleanupPreflightReport(
  fixture: Fixture,
  update: (report: Record<string, unknown>) => void,
) {
  const pin = fixture.receipt.capacityPreflight;
  const accepted = JSON.parse(fs.readFileSync(pin.path, "utf8")) as Record<string, unknown>;
  const cleanupPin = z
    .object({ path: z.string(), size: z.number(), sha256: z.string() })
    .parse(accepted.cleanupObservation);
  accepted.cleanupObservation = updateCapacityEvidenceReport(cleanupPin, update);
  fixture.receipt.capacityPreflight = writeFile(pin.path, JSON.stringify(accepted));
}

function capacityData(
  fixture: Fixture,
  phase: "capacityBefore" | "capacityAfter",
  change: (report: Record<string, unknown>) => void,
) {
  const pin = fixture.observation[phase];
  const report = z.record(z.string(), z.unknown()).parse(read(pin));
  change(report);
  fixture.observation[phase] = writeFile(pin.path, JSON.stringify(report));
  observation(fixture);
}

for (const phase of ["capacityBefore", "capacityAfter"] as const) {
  for (const [name, change] of [
    [
      "missing available metric",
      (r: Record<string, unknown>) => {
        delete r.metric_available;
      },
    ],
    [
      "unavailable metric",
      (r: Record<string, unknown>) => {
        r.metric_available = false;
      },
    ],
    [
      "different metric",
      (r: Record<string, unknown>) => {
        r.metric = "logical-bytes";
      },
    ],
    [
      "different metric version",
      (r: Record<string, unknown>) => {
        r.metric_version = 2;
      },
    ],
    [
      "raised baseline",
      (r: Record<string, unknown>) => {
        r.baseline_bytes = 2363047937;
      },
    ],
    [
      "raised factor",
      (r: Record<string, unknown>) => {
        r.factor = 3;
      },
    ],
    [
      "raised threshold",
      (r: Record<string, unknown>) => {
        r.threshold_bytes = 4726095873;
      },
    ],
    [
      "over threshold",
      (r: Record<string, unknown>) => {
        r.current_bytes = 4726095873;
      },
    ],
    [
      "negative measurement",
      (r: Record<string, unknown>) => {
        r.current_bytes = -1;
      },
    ],
    [
      "over-capacity claim",
      (r: Record<string, unknown>) => {
        r.over_capacity = true;
      },
    ],
    [
      "different target",
      (r: Record<string, unknown>) => {
        r.target = `${r.target}/other`;
      },
    ],
    [
      "different state",
      (r: Record<string, unknown>) => {
        r.state = `${r.state}.other`;
      },
    ],
  ] as const)
    test(`managed ${phase} rejects ${name} with a current raw pin`, () => {
      const f = createManagedReaderFixture();
      try {
        capacityData(f, phase, change);
        expect(() => verify(f)).toThrow();
      } finally {
        fs.rmSync(f.root, { recursive: true, force: true });
      }
    });
  test(`managed ${phase} rejects raw bytes changed after its source pin`, () => {
    const f = createManagedReaderFixture();
    try {
      fs.appendFileSync(f.observation[phase].path, "\n");
      expect(() => verify(f)).toThrow(/artifact role changed/);
    } finally {
      fs.rmSync(f.root, { recursive: true, force: true });
    }
  });
}

for (const [field, value] of [
  ["removed", [{}]],
  ["cleanup_status", "completed"],
  ["actual_removed_bytes", 1],
  ["post_clean_bytes", 1],
  ["remaining_over_capacity", true],
  ["baseline_eligible", true],
] as const)
  test(`managed before execution rejects changed ${field}`, () => {
    const f = createManagedReaderFixture();
    try {
      capacityData(f, "capacityBefore", (report) => {
        report[field] = value;
      });
      expect(() => verify(f)).toThrow();
    } finally {
      fs.rmSync(f.root, { recursive: true, force: true });
    }
  });

for (const field of [
  "removed",
  "cleanup_status",
  "actual_removed_bytes",
  "post_clean_bytes",
  "remaining_over_capacity",
  "baseline_eligible",
])
  test(`managed after measurement rejects injected execution field ${field}`, () => {
    const f = createManagedReaderFixture();
    try {
      const execution = z.record(z.string(), z.unknown()).parse(read(f.observation.capacityBefore));
      capacityData(f, "capacityAfter", (report) => {
        report[field] = execution[field];
      });
      expect(() => verify(f)).toThrow(/Unrecognized key/);
    } finally {
      fs.rmSync(f.root, { recursive: true, force: true });
    }
  });

test("managed phase pair accepts execution before and plain measurement after in the synthetic reader", () => {
  const f = createManagedReaderFixture();
  try {
    expect(
      Object.hasOwn(
        z.record(z.string(), z.unknown()).parse(read(f.observation.capacityAfter)),
        "removed",
      ),
    ).toBe(false);
    verifyManagedCapacity(f.root, f.observation.capacityBefore, f.observation.capacityAfter);
    expect(verify(f).sourceFileCount).toBe(f.receipt.snapshotAfter.source.length);
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
});

test("managed v3 receipt binds the Rustup launchers and no-clean preflight FileRecord", () => {
  const f = createManagedReaderFixture();
  try {
    expect(f.receipt.schema).toBe("krr-native-cli-managed-build-v3");
    expect(f.receipt.capacityPreflight.path).toBe(
      path.join(path.dirname(f.receiptPath), "capacity-preflight.json"),
    );
    verify(f);
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
});

test("managed observer rejects a Rustup query rewritten as a direct executable call", () => {
  const f = createManagedReaderFixture();
  try {
    verify(f);
    const query = f.observation.rustupResolutionQueries[0];
    if (query === undefined) throw new Error("Rustup query fixture missing");
    const directArgv = [f.receipt.snapshotAfter.tools.cargo.path, "which", "cargo"];
    query.observation.argv = directArgv;
    const lifecycle = JSON.parse(fs.readFileSync(query.lifecycle.path, "utf8")) as {
      argv: string[];
    };
    lifecycle.argv = directArgv;
    query.lifecycle = writeFile(query.lifecycle.path, JSON.stringify(lifecycle));
    observation(f);
    expect(() => verify(f)).toThrow(/Rustup query argv/);
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
});

test("managed preflight rejects cleanup attempts even when all report pins agree", () => {
  const f = createManagedReaderFixture();
  try {
    updateAcceptedPreflightReport(f, (report) => {
      report.cleanup_attempts = [{ exit_code: 0, dry_run: false }];
    });
    expect(() => verify(f)).toThrow(/accepted capacity preflight includes cleanup/);
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
});

for (const [label, update] of [
  [
    "observer argv",
    (closure: Record<string, unknown>) => {
      closure.argv = [...z.array(z.string()).parse(closure.argv), "--unexpected"];
    },
  ],
  [
    "observer tool pin",
    (closure: Record<string, unknown>) => {
      closure.toolBefore = closure.stdout;
    },
  ],
  [
    "observer raw pin",
    (closure: Record<string, unknown>) => {
      const stdout = z.record(z.string(), z.unknown()).parse(closure.stdout);
      stdout.path = `${z.string().parse(stdout.path)}.other`;
      closure.stdout = stdout;
    },
  ],
  [
    "owner process identity",
    (closure: Record<string, unknown>, ownerPid: number) => {
      const identity = z.record(z.string(), z.unknown()).parse(closure.identity);
      identity.pid = ownerPid;
      identity.sessionId = identity.pid;
      identity.processGroupId = identity.pid;
      closure.identity = identity;
    },
  ],
  [
    "owner time boundary",
    (closure: Record<string, unknown>) => {
      closure.startedNs = "0";
    },
  ],
  [
    "environment pin",
    (closure: Record<string, unknown>) => {
      const environment = z.record(z.string(), z.string()).parse(closure.environment);
      environment.EXTRA = "changed";
      closure.environment = environment;
      closure.environmentSha256 = digestStringRecord(environment);
    },
  ],
] as const)
  test(`managed preflight raw-FD closure rejects ${label}`, () => {
    const f = createManagedReaderFixture();
    try {
      updatePreflightClosure(f, update);
      expect(() => verify(f)).toThrow();
    } finally {
      fs.rmSync(f.root, { recursive: true, force: true });
    }
  });

test("managed v2 receipt binds cleanup and fresh preflight observations", () => {
  const f = createManagedReaderFixture(true);
  try {
    const preflight = JSON.parse(fs.readFileSync(f.receipt.capacityPreflight.path, "utf8")) as {
      cleanupObservation: FileRecord;
      refreshObservation: FileRecord;
    };
    expect(preflight.cleanupObservation.path).toBe(
      path.join(path.dirname(f.receiptPath), "capacity-preflight-cleanup-observation.json"),
    );
    expect(preflight.refreshObservation.path).toBe(
      path.join(path.dirname(f.receiptPath), "capacity-preflight-refresh-observation.json"),
    );
    expect(verify(f).sourceFileCount).toBe(f.receipt.snapshotAfter.source.length);
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
});

test("managed cleanup preflight rejects negative post-clean capacity with bound pins", () => {
  const f = createManagedReaderFixture(true);
  try {
    updateCleanupPreflightReport(f, (report) => {
      report.post_clean_bytes = -1;
    });
    expect(() => verify(f)).toThrow(/cleanup capacity preflight report differs/);
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
});

test("managed v2 receipt rejects a cleanup pin from a different attempt", () => {
  const f = createManagedReaderFixture(true);
  const other = createManagedReaderFixture(true);
  try {
    f.receipt.capacityPreflight = other.receipt.capacityPreflight;
    expect(() => verify(f)).toThrow(/capacity preflight evidence path differs/);
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
    fs.rmSync(other.root, { recursive: true, force: true });
  }
});

test("managed v1 receipt remains outside current v2 acceptance", () => {
  const f = createManagedReaderFixture();
  try {
    Object.assign(f.receipt, { schema: "krr-native-cli-managed-build-v1" });
    expect(() => verify(f)).toThrow();
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
});

test("managed preflight accepts the producer's validated retained debug profile", () => {
  const f = createManagedReaderFixture();
  try {
    updatePreflightCommand(f, (argv) => [
      ...argv,
      "--retain-profile",
      path.join(f.root, "target/debug"),
    ]);
    expect(verify(f).sourceFileCount).toBe(f.receipt.snapshotAfter.source.length);
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
});

for (const [label, update] of [
  ["duplicate root option", (argv: string[]) => [...argv, "--root", "/other"]],
  ["duplicate target option", (argv: string[]) => [...argv, "--target", "/other/target"]],
  ["extra dry-run option", (argv: string[]) => [...argv, "--dry-run"]],
  [
    "duplicate retain profile option",
    (argv: string[]) => {
      const targetIndex = argv.indexOf("--target");
      const target = argv[targetIndex + 1];
      if (target === undefined) return argv;
      return [
        ...argv,
        "--retain-profile",
        path.join(target, "debug"),
        "--retain-profile",
        path.join("/other", "target/debug"),
      ];
    },
  ],
] as const)
  test(`managed preflight rejects ${label}`, () => {
    const f = createManagedReaderFixture();
    try {
      verify(f);
      updatePreflightCommand(f, update);
      expect(() => verify(f)).toThrow(/capacity preflight admission argv differs/);
    } finally {
      fs.rmSync(f.root, { recursive: true, force: true });
    }
  });

for (const [label, mutate] of [
  [
    "missing preflight pin",
    (f: Fixture) => Object.assign(f.receipt, { capacityPreflight: undefined }),
  ],
  [
    "accepted preflight file changed after its receipt pin",
    (f: Fixture) => fs.appendFileSync(f.receipt.capacityPreflight.path, "\n"),
  ],
  [
    "wrong preflight path",
    (f: Fixture) =>
      Object.assign(f.receipt, {
        capacityPreflight: {
          ...f.receipt.capacityPreflight,
          path: path.join(path.dirname(f.receiptPath), "other-preflight.json"),
        },
      }),
  ],
  [
    "missing cleanup sidecar pin",
    (f: Fixture) =>
      updateCapacityPreflight(f, (value) => {
        value.cleanupObservation = null;
      }),
  ],
  [
    "cleanup sidecar changed after pin",
    (f: Fixture) => {
      const preflight = JSON.parse(fs.readFileSync(f.receipt.capacityPreflight.path, "utf8")) as {
        cleanupObservation: FileRecord;
      };
      fs.appendFileSync(preflight.cleanupObservation.path, "\n");
    },
  ],
  [
    "cleanup sidecar bound to refresh path",
    (f: Fixture) =>
      updateCapacityPreflight(f, (value) => {
        const refresh = value.refreshObservation as FileRecord;
        value.cleanupObservation = refresh;
      }),
  ],
] as const) {
  test(`managed v2 receipt rejects ${label}`, () => {
    const f = createManagedReaderFixture(true);
    try {
      verify(f);
      mutate(f);
      expect(() => verify(f)).toThrow();
    } finally {
      fs.rmSync(f.root, { recursive: true, force: true });
    }
  });
}

test("managed phase pair accepts a clean bootstrap with an empty eligible baseline", () => {
  const f = createManagedReaderFixture();
  try {
    capacityData(f, "capacityBefore", (report) => {
      report.current_bytes = 0;
      report.baseline_eligible = true;
    });
    verifyManagedCapacity(f.root, f.observation.capacityBefore, f.observation.capacityAfter);
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
});

test("full managed reader accepts an explicitly synthetic data graph; native remains unproved", () => {
  const fixture = createManagedReaderFixture();
  try {
    const result = verify(fixture);
    expect(result.sourceFileCount).toBe(fixture.receipt.snapshotAfter.source.length);
    expect(result.binarySha256).toBe(fixture.receipt.binary.sha256);
    expect(fixture.receipt.scope.effectiveCompilerReadsProved).toBe(false);
    expect(fixture.receipt.scope.registryAtomicityProved).toBe(false);
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("managed reader rejects a legacy empty Rust facts host receipt", () => {
  const fixture = createManagedReaderFixture();
  try {
    fixture.receipt.hostPrebuild.receipt = writeFile(
      fixture.receipt.hostPrebuild.receipt.path,
      JSON.stringify({ schema: "krr-rust-facts-host-v1", status: "prepared" }),
    );
    expect(() => verify(fixture)).toThrow(/krr-rust-facts-host\/v2/);
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("managed current source query uses pinned Git despite a fake parent PATH", () => {
  const fixture = createManagedReaderFixture();
  try {
    const directory = path.join(fixture.root, "tmp/fake-parent");
    fs.mkdirSync(directory);
    const fake = path.join(directory, "git");
    fs.writeFileSync(fake, "#!/bin/sh\nexit 99\n");
    fs.chmodSync(fake, 0o700);
    const snapshot = fixture.receipt.snapshotAfter;
    const environment = {
      ...snapshot.environment,
      PATH: `${directory}${path.delimiter}${snapshot.environment.PATH ?? ""}`,
    };
    expect(() =>
      assertCurrentGitSourceSet(
        fixture.root,
        snapshot.source,
        snapshot.tools.git.path,
        environment,
      ),
    ).not.toThrow();
    expect(verify(fixture).sourceFileCount).toBe(fixture.receipt.snapshotAfter.source.length);
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

const negatives: Array<[string, (fixture: Fixture) => void, RegExp]> = [
  [
    "current Git-visible input set grows",
    (f) => fs.writeFileSync(path.join(f.root, "additional.rs"), "new source"),
    /differs from current tracked source/,
  ],
  [
    "noncanonical request alias before reading",
    (f) => {
      f.receipt.request.path = `${path.dirname(f.receiptPath)}/../${f.receipt.attemptToken}/request.json`;
    },
    /admission\/request path boundary/,
  ],
  [
    "selected tool bytes drift",
    (f) => fs.appendFileSync(f.receipt.snapshotAfter.tools.cargo.path, "changed"),
    /managed input changed/,
  ],
  [
    "current Rust facts host receipt bytes drift",
    (f) => fs.appendFileSync(f.receipt.hostPrebuild.receipt.path, "changed"),
    /managed Rust facts host receipt changed/,
  ],
  [
    "saved capacity baseline is raised with a current physical pin",
    (f) => {
      const pin = f.receipt.snapshotAfter.capacityAdmission.state;
      const state = JSON.parse(fs.readFileSync(pin.path, "utf8")) as { [key: string]: unknown };
      state.baseline_bytes = 2363047937;
      const current = writeFile(pin.path, JSON.stringify(state));
      f.receipt.snapshotBefore.capacityAdmission.state = current;
      f.receipt.snapshotAfter.capacityAdmission.state = current;
    },
    /baseline_bytes/,
  ],
  [
    "unsupported Cargo environment is added with matching endpoint digest",
    (f) => {
      for (const snapshot of [f.receipt.snapshotBefore, f.receipt.snapshotAfter]) {
        snapshot.environment.RUSTC = "/unadmitted/compiler";
        snapshot.environmentSha256 = digestStringRecord(snapshot.environment);
      }
    },
    /unsupported Cargo environment/,
  ],
  ...(
    [
      ["CARGO", "cargo"],
      ["CARGO_BUILD_JOBS", "3"],
      ["TEST_THREADS", "0"],
    ] as const
  ).map(([key, value]): [string, (fixture: Fixture) => void, RegExp] => [
    `incorrect ${key} with matching endpoint digest`,
    (f) => {
      for (const snapshot of [f.receipt.snapshotBefore, f.receipt.snapshotAfter]) {
        snapshot.environment[key] = value;
        snapshot.environmentSha256 = digestStringRecord(snapshot.environment);
      }
    },
    /Just (Cargo|concurrency) export/,
  ]),
  [
    "capacity crosses fixed threshold with current raw pin",
    (f) => {
      const pin = f.observation.capacityBefore;
      const report = JSON.parse(fs.readFileSync(pin.path, "utf8")) as { [key: string]: unknown };
      report.current_bytes = 4726095873;
      f.observation.capacityBefore = writeFile(pin.path, JSON.stringify(report));
      observation(f);
    },
    /under-threshold repository target/,
  ],
  [
    "changed current source bytes",
    (f) =>
      fs.appendFileSync(
        path.join(f.root, "crates/katana-render-runtime-cli/src/main.rs"),
        "changed",
      ),
    /managed current source changed/,
  ],
  [
    "nonce moved to another attempt",
    (f) => {
      f.receipt.attemptToken = "a".repeat(64);
    },
    /attempt path differs/,
  ],
  [
    "Cargo PID aliases metadata owner",
    (f) => {
      f.observation.ownedCargo.pid = 101;
      observation(f);
    },
    /Cargo PID\/group\/time/,
  ],
  [
    "Cargo group leaves outer owner",
    (f) => {
      f.observation.ownedCargo.processGroupId = 103;
      observation(f);
    },
    /Cargo PID\/group\/time/,
  ],
  [
    "current protected reference changed",
    (f) =>
      fs.appendFileSync(f.receipt.snapshotAfter.capacityAdmission.refs[0]?.path ?? "", "changed"),
    /protected reference changed/,
  ],
  [
    "required configuration inventory omitted",
    (f) => {
      f.observation.pinsBefore.unsupportedCargoConfigsAbsent = [];
      f.observation.pinsAfter.unsupportedCargoConfigsAbsent = [];
      observation(f);
    },
    /configuration absence inventory/,
  ],
  [
    "outer group closure false with current lifecycle hash",
    (f) => {
      const lifecycle = JSON.parse(fs.readFileSync(f.receipt.buildLifecycle.path, "utf8")) as {
        [key: string]: unknown;
      };
      lifecycle.groupClosed = false;
      f.receipt.buildLifecycle = writeFile(
        f.receipt.buildLifecycle.path,
        JSON.stringify(lifecycle),
      );
    },
    /groupClosed/,
  ],
  [
    "outer build environment loses request SHA",
    (f) => {
      delete f.receipt.buildObservation.environment.KRR_NATIVE_CLI_GUARD_REQUEST_SHA256;
    },
    /exact outer Just environment/,
  ],
  [
    "otherwise valid Cargo artifact has boolean features",
    (f) => {
      f.observation.artifact.features = true;
      f.observation.ownedCargo.stdout = writeFile(
        f.observation.ownedCargo.stdout.path,
        `${JSON.stringify(f.observation.artifact)}\n${JSON.stringify(f.observation.buildFinished)}\n`,
      );
      observation(f);
    },
    /features/,
  ],
  [
    "escaped duplicate features in physical raw Cargo event",
    (f) => {
      const raw = JSON.stringify(f.observation.artifact);
      f.observation.ownedCargo.stdout = writeFile(
        f.observation.ownedCargo.stdout.path,
        `${duplicateManagedFixtureJson(raw, "features", true)}\n${JSON.stringify(f.observation.buildFinished)}\n`,
      );
      observation(f);
    },
    /Duplicate managed JSON key: features/,
  ],
  [
    "build-finished is not the terminal event",
    (f) => {
      f.observation.ownedCargo.stdout = writeFile(
        f.observation.ownedCargo.stdout.path,
        `${JSON.stringify(f.observation.artifact)}\n${JSON.stringify(f.observation.buildFinished)}\n${JSON.stringify({ reason: "compiler-message", success: true })}\n`,
      );
      observation(f);
    },
    /terminal success/,
  ],
];
for (const [label, mutate, expected] of negatives) {
  test(`full managed reader rejects ${label} after the same valid baseline`, () => {
    const fixture = createManagedReaderFixture();
    try {
      verify(fixture);
      mutate(fixture);
      expect(() => verify(fixture)).toThrow(expected);
    } finally {
      fs.rmSync(fixture.root, { recursive: true, force: true });
    }
  });
}

for (const escaped of [false, true]) {
  test(`full managed reader rejects duplicated request schema; escaped=${escaped}`, () => {
    const f = createManagedReaderFixture();
    try {
      verify(f);
      f.receipt.request = writeFile(
        f.receipt.request.path,
        duplicateManagedFixtureJson(
          fs.readFileSync(f.receipt.request.path, "utf8"),
          "schema",
          escaped,
        ),
      );
      f.observation.request = f.receipt.request;
      observation(f);
      const env = f.receipt.buildObservation.environment;
      env.KRR_NATIVE_CLI_GUARD_REQUEST_SHA256 = f.receipt.request.sha256;
      f.receipt.buildObservation.environmentSha256 = digestStringRecord(env);
      const life = JSON.parse(fs.readFileSync(f.receipt.buildLifecycle.path, "utf8"));
      life.nativeRawFdClosure.environment = env;
      life.nativeRawFdClosure.environmentSha256 = digestStringRecord(env);
      f.receipt.buildLifecycle = writeFile(f.receipt.buildLifecycle.path, JSON.stringify(life));
      expect(() => verify(f)).toThrow(/Duplicate managed JSON key: schema/);
    } finally {
      fs.rmSync(f.root, { recursive: true, force: true });
    }
  });
  test(`managed declaration cannot escape to legacy through duplicate schema; escaped=${escaped}`, () => {
    const f = createManagedReaderFixture();
    try {
      verify(f);
      const raw = JSON.stringify(f.receipt);
      const spelling = escaped ? '"\\u0073chema"' : '"schema"';
      writeFile(f.receiptPath, `${raw.slice(0, -1)},${spelling}:"krr-native-build-receipt-v1"}`);
      expect(() =>
        verifyNativeBuildReceipt(f.receiptPath, f.binaryPath, f.runtimePath, f.root),
      ).toThrow(/Duplicate managed JSON key: schema/);
      expect(() =>
        parseManagedJson(String.raw`{"environment":{"PATH":"a","P\u0041TH":"b"}}`, "unit"),
      ).toThrow(/Duplicate managed JSON key: PATH/);
    } finally {
      fs.rmSync(f.root, { recursive: true, force: true });
    }
  });
}

test("every managed JSON file boundary rejects duplicated decoded keys with physical pins", () => {
  const f = createManagedReaderFixture();
  try {
    verify(f);
    const paths = [
      f.receipt.observation.path,
      f.receipt.buildLifecycle.path,
      f.request.metadata.path,
      f.request.metadataObservation.path,
      f.request.sourceManifest.path,
      f.receipt.expectedSource.path,
      f.receipt.snapshotAfter.capacityAdmission.state.path,
      f.receipt.expectedProtectedRegistry.path,
      f.observation.capacityBefore.path,
    ];
    for (const file of paths) {
      const raw = fs.readFileSync(file, "utf8"),
        value = JSON.parse(raw) as { [key: string]: unknown };
      const key = Object.keys(value)[0];
      if (key === undefined) throw new Error("Unit boundary empty");
      const pin = writeFile(file, duplicateManagedFixtureJson(raw, key, true));
      expect(() => read(pin)).toThrow(/Duplicate managed JSON key/);
    }
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
});

// 実producerの欠落を補完せず、保存済みrawbytesを拒否する回帰として固定する。
const capturedActualCapacity = [
  {
    phase: "before",
    raw: '{\n  "actual_removed_bytes": 0,\n  "baseline_bytes": 2363047936,\n  "baseline_eligible": false,\n  "cleanup_status": "not-needed",\n  "current_bytes": 4597989376,\n  "factor": 2.0,\n  "metric": "unique-inode-allocated-bytes",\n  "metric_version": 1,\n  "over_capacity": false,\n  "post_clean_bytes": null,\n  "remaining_over_capacity": false,\n  "removed": [],\n  "state": "/Users/hiroyuki_furuno/works/private/katana-render-runtime/tmp/.cargo-capacity-guard.json",\n  "target": "/Users/hiroyuki_furuno/works/private/katana-render-runtime/target",\n  "threshold_bytes": 4726095872.0\n}\n',
    sha256: "9b48d747aa3278f36e160b2c3aa3bd372be5fa663e0c4d629df39914647ef8a4",
    size: 570,
  },
  {
    phase: "after",
    raw: '{\n  "baseline_bytes": 2363047936,\n  "current_bytes": 4597989376,\n  "factor": 2.0,\n  "metric": "unique-inode-allocated-bytes",\n  "metric_version": 1,\n  "over_capacity": false,\n  "state": "/Users/hiroyuki_furuno/works/private/katana-render-runtime/tmp/.cargo-capacity-guard.json",\n  "target": "/Users/hiroyuki_furuno/works/private/katana-render-runtime/target",\n  "threshold_bytes": 4726095872.0\n}\n',
    sha256: "d5124ec99656dd96958a346cd32717d1740ce4dadab4963b7805e2ff6b7a85db",
    size: 396,
  },
] as const;
test("captured actual managed capacity raw keeps missing availability rejected without fabricated fields", () => {
  const f = createManagedReaderFixture();
  try {
    const pins = capturedActualCapacity.map((capture) => {
      const pin = writeFile(
        path.join(f.root, "tmp", `captured-capacity.${capture.phase}.json`),
        capture.raw,
      );
      expect([pin.sha256, pin.size]).toEqual([capture.sha256, capture.size]);
      expect(
        Object.hasOwn(z.record(z.string(), z.unknown()).parse(read(pin)), "metric_available"),
      ).toBe(false);
      return pin;
    });
    const before = pins[0],
      after = pins[1];
    if (!before || !after) throw new Error("Missing captured actual capacity pair");
    const actualRoot = path.dirname(z.object({ target: z.string() }).parse(read(after)).target);
    expect(() => verifyManagedCapacity(actualRoot, before, after)).toThrow(
      /under-threshold repository target/,
    );
    expect(() => validateCapacityReport(read(after), "captured actual after", actualRoot)).toThrow(
      /under-threshold repository target/,
    );
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
});
