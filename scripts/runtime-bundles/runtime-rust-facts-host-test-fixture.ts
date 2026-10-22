import fs from "node:fs";
import path from "node:path";
import type { RustFactsHostInputs } from "./runtime-rust-facts-host-inputs";
import { hostFileRecord } from "./runtime-rust-facts-host-inputs";

export function hostFixtureInputs(root: string) {
  const metadata = {
    target_directory: path.join(root, "target"),
    packages: [
      {
        id: "host",
        name: "kdr-linter",
        manifest_path: path.join(root, "crates/kdr-linter/Cargo.toml"),
      },
      {
        id: "dependency",
        name: "host-dependency",
        manifest_path: path.join(root, "dependency/Cargo.toml"),
      },
    ],
    resolve: {
      nodes: [
        { id: "host", dependencies: ["dependency"] },
        { id: "dependency", dependencies: [] },
      ],
    },
  };
  const names = ["just", "cargo", "rustc", "rtk", "python3", "bun"] as const;
  const tools = Object.fromEntries(
    names.map((name) => {
      const located = name === "bun" ? process.execPath : Bun.which(name);
      if (!located) throw new Error(`missing fixture tool ${name}`);
      const record = hostFileRecord(fs.realpathSync(located));
      return [name, { launcher: record, executable: record }];
    }),
  ) as RustFactsHostInputs["tools"];
  return { metadata, tools };
}
