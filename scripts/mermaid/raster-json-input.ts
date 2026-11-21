import type { JsonValue } from "./mermaid-pair-manifest";

export type JsonInput = JsonValue | undefined | JsonInput[] | JsonObject;
export type JsonObject = { [key: string]: JsonInput };
export type JsonFields<T> = { [Key in keyof T]?: T[Key] | JsonInput };
