import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { type CDPSession, chromium } from "playwright";

type JsonRecord = Record<string, unknown>;
export type Picture = { bytes: Buffer; events: JsonRecord[]; sha256: string };

function record(value: unknown): JsonRecord {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error("invalid trace object");
  return value as JsonRecord;
}

function sha256(bytes: Buffer): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function pictureStrings(value: unknown): string[] {
  if (Array.isArray(value)) return value.flatMap(pictureStrings);
  if (typeof value !== "object" || value === null) return [];
  return Object.entries(value).flatMap(([key, item]) => {
    if (key !== "skp64") return pictureStrings(item);
    if (typeof item !== "string") throw new Error("invalid skp64 value");
    return [item];
  });
}

function withoutPictureData(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(withoutPictureData);
  if (typeof value !== "object" || value === null) return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(([key]) => key !== "skp64")
      .map(([key, item]) => [key, withoutPictureData(item)]),
  );
}

export function extractPictures(trace: unknown): Picture[] {
  const envelope = record(trace);
  if (!Array.isArray(envelope.traceEvents)) throw new Error("traceEvents missing");
  const pictures = new Map<string, Picture>();
  for (const value of envelope.traceEvents) {
    const event = record(value);
    for (const encoded of pictureStrings(event.args)) {
      if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(encoded))
        throw new Error("invalid skp64 encoding");
      const bytes = Buffer.from(encoded, "base64");
      if (bytes.length === 0 || bytes.toString("base64") !== encoded)
        throw new Error("empty or noncanonical skp64");
      const digest = sha256(bytes);
      const metadata = { ...event, args: withoutPictureData(event.args) };
      const existing = pictures.get(digest);
      if (existing) existing.events.push(metadata);
      else pictures.set(digest, { bytes, sha256: digest, events: [metadata] });
    }
  }
  return [...pictures.values()];
}

type Request = {
  input: string;
  inputSha256: string;
  chrome: string;
  chromeSha256: string;
  output: string;
  width: number;
  height: number;
  dpr: number;
};

function parseArgs(args: string[]): Request {
  const keys = [
    "input",
    "input-sha256",
    "chrome",
    "chrome-sha256",
    "output",
    "width",
    "height",
    "dpr",
  ];
  if (args.length !== keys.length * 2) throw new Error("all diagnostic options are required once");
  const values = new Map<string, string>();
  for (let i = 0; i < args.length; i += 2) {
    const key = args[i]?.replace(/^--/, "");
    const value = args[i + 1];
    if (!key || !keys.includes(key) || values.has(key) || !value)
      throw new Error("invalid diagnostic option");
    values.set(key, value);
  }
  const get = (key: string): string => {
    const value = values.get(key);
    if (!value) throw new Error(`missing ${key}`);
    return value;
  };
  const width = Number(get("width")),
    height = Number(get("height")),
    dpr = Number(get("dpr"));
  if (
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height) ||
    width <= 0 ||
    height <= 0 ||
    !Number.isFinite(dpr) ||
    dpr <= 0
  )
    throw new Error("invalid viewport");
  const input = get("input"),
    chrome = get("chrome"),
    output = get("output");
  if (![input, chrome, output].every(path.isAbsolute)) throw new Error("paths must be absolute");
  const inputSha256 = get("input-sha256"),
    chromeSha256 = get("chrome-sha256");
  if (![inputSha256, chromeSha256].every((value) => /^[a-f0-9]{64}$/.test(value)))
    throw new Error("invalid SHA256");
  return { input, inputSha256, chrome, chromeSha256, output, width, height, dpr };
}

async function readStream(cdp: CDPSession, handle: string): Promise<Buffer> {
  const chunks: Buffer[] = [];
  while (true) {
    const result = await cdp.send("IO.read", { handle });
    chunks.push(Buffer.from(result.data, result.base64Encoded ? "base64" : "utf8"));
    if (result.eof) break;
  }
  await cdp.send("IO.close", { handle });
  return Buffer.concat(chunks);
}

function requireInputPins(request: Request): void {
  if (
    sha256(fs.readFileSync(request.input)) !== request.inputSha256 ||
    sha256(fs.readFileSync(request.chrome)) !== request.chromeSha256
  )
    throw new Error("input or Chrome SHA differs");
}

function saveCapture(
  request: Request,
  screenshot: Buffer,
  trace: Buffer,
  browserVersion: string,
): void {
  fs.writeFileSync(path.join(request.output, "frame.png"), screenshot, { flag: "wx" });
  fs.writeFileSync(path.join(request.output, "trace.json"), trace, { flag: "wx" });
  const rows = extractPictures(JSON.parse(trace.toString("utf8")) as unknown).map(
    (picture, index) => {
      const name = `picture-${index}.skp`;
      fs.writeFileSync(path.join(request.output, name), picture.bytes, { flag: "wx" });
      return { name, sha256: picture.sha256, events: picture.events };
    },
  );
  const report = {
    diagnosticOnly: true,
    formalAcceptance: false,
    request,
    browserVersion,
    screenshotSha256: sha256(screenshot),
    traceSha256: sha256(trace),
    pictures: rows,
    status: rows.length ? "pictures-observed" : "not-established",
    reason: rows.length ? null : "trace contained no skp64 picture payload",
    coordinateContract: "picture-local; screen transform and font identity not established",
  };
  fs.writeFileSync(path.join(request.output, "report.json"), JSON.stringify(report, null, 2), {
    flag: "wx",
  });
}

async function capture(request: Request): Promise<void> {
  requireInputPins(request);
  fs.mkdirSync(request.output);
  const browser = await chromium.launch({ executablePath: request.chrome, headless: true });
  try {
    const context = await browser.newContext({
      viewport: { width: request.width, height: request.height },
      deviceScaleFactor: request.dpr,
    });
    const page = await context.newPage();
    const cdp = await context.newCDPSession(page);
    await cdp.send("LayerTree.enable");
    const completed = new Promise<string>((resolve, reject) =>
      cdp.once("Tracing.tracingComplete", (event) => {
        if (event.stream) resolve(event.stream);
        else reject(new Error("trace stream missing"));
      }),
    );
    await cdp.send("Tracing.start", {
      categories:
        "devtools.timeline,disabled-by-default-devtools.timeline.picture,disabled-by-default-devtools.timeline.layers,disabled-by-default-cc.debug.picture,disabled-by-default-cc.debug.display_items",
      transferMode: "ReturnAsStream",
    });
    await page.goto(pathToFileURL(request.input).href, { waitUntil: "load" });
    await page.evaluate(async () => {
      await document.fonts.ready;
    });
    const screenshot = await page.screenshot();
    await cdp.send("Tracing.end");
    const trace = await readStream(cdp, await completed);
    requireInputPins(request);
    saveCapture(request, screenshot, trace, browser.version());
    await context.close();
  } finally {
    await browser.close();
  }
}

if (import.meta.main) await capture(parseArgs(process.argv.slice(2)));
