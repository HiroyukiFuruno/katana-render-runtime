import { parseRuntimeBundleMode } from "./runtime-bundle-cli";
import { RuntimeBundleCommand } from "./runtime-bundle-command";

await new RuntimeBundleCommand(process.cwd(), parseRuntimeBundleMode(process.argv.slice(2))).run();
