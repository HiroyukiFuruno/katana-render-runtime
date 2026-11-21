import { runInThisContext } from "node:vm";
import { installDrawioDeterminism } from "./official-runtime-determinism";

export type DeterminismChildMode = "date-call" | "date-string" | "locale" | "formatted";

function sample(mode: string | undefined): string {
  switch (mode) {
    case "date-call":
      return Date();
    case "date-string":
      return new Date("2024-02-03T04:05:06.000Z").toString();
    case "locale":
      return JSON.stringify({
        date: new Date().toLocaleDateString(),
        string: new Date().toLocaleString(),
        time: new Date().toLocaleTimeString(),
      });
    case "formatted": {
      const date = new Date();
      const pad = (value: number) => String(value).padStart(2, "0");
      return (
        [date.getFullYear(), pad(date.getMonth() + 1), pad(date.getDate())].join("-") +
        "T" +
        [pad(date.getHours()), pad(date.getMinutes()), pad(date.getSeconds())].join(":") +
        "Z"
      );
    }
    default:
      throw new Error(`Unsupported Draw.io determinism child mode: ${String(mode)}`);
  }
}

if (import.meta.main) {
  // browser注入と同じ関数の文字列化を経て、独立した子プロセス内で実行する。
  runInThisContext(`(${installDrawioDeterminism.toString()})();`);
  process.stdout.write(sample(process.argv[2]));
}
