import { type Expression, parseSync } from "@swc/core";
import { type JsonValue, parseJsonValue } from "./mermaid-pair-manifest";

function expression(serialized: string): Expression {
  const statement = parseSync(`(${serialized}\n)`, { syntax: "ecmascript" }).body[0];
  if (statement?.type !== "ExpressionStatement") throw new Error("Invalid managed JSON AST");
  return statement.expression.type === "ParenthesisExpression"
    ? statement.expression.expression
    : statement.expression;
}

function decodedString(value: Extract<Expression, { type: "StringLiteral" }>): string {
  const decoded = parseJsonValue(value.raw ?? JSON.stringify(value.value), "managed JSON key");
  if (typeof decoded !== "string") throw new Error("Invalid managed JSON string");
  return decoded;
}

function objectValues(value: Extract<Expression, { type: "ObjectExpression" }>): Expression[] {
  const keys = new Set<string>();
  return value.properties.map((property) => {
    if (property.type !== "KeyValueProperty" || property.key.type !== "StringLiteral") {
      throw new Error("Invalid managed JSON object AST");
    }
    const key = decodedString(property.key);
    if (keys.has(key)) throw new Error(`Duplicate managed JSON key: ${key}`);
    keys.add(key);
    return property.value;
  });
}

function rejectDuplicates(value: Expression): void {
  if (value.type === "ObjectExpression") {
    for (const child of objectValues(value)) rejectDuplicates(child);
  } else if (value.type === "ArrayExpression") {
    for (const element of value.elements) {
      if (element != null) rejectDuplicates(element.expression);
    }
  }
}

export function parseManagedJson(serialized: string, label: string): JsonValue {
  // JSON syntax/value検査は既存reader、decoded key検査は既存SWCに任せる。
  const value = parseJsonValue(serialized, label);
  rejectDuplicates(expression(serialized));
  return value;
}

export function declaresManagedReceipt(serialized: string, schema: string): boolean {
  const value = expression(serialized);
  return (
    value.type === "ObjectExpression" &&
    value.properties.some(
      (property) =>
        property.type === "KeyValueProperty" &&
        property.key.type === "StringLiteral" &&
        decodedString(property.key) === "schema" &&
        property.value.type === "StringLiteral" &&
        decodedString(property.value) === schema,
    )
  );
}
