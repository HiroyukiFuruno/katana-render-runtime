export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

export function jsonRecords(
  value: JsonValue | undefined,
  name: string,
): Array<{ [key: string]: JsonValue }> {
  if (
    !Array.isArray(value) ||
    value.some((item) => typeof item !== "object" || item === null || Array.isArray(item))
  )
    throw new Error(`invalid ${name}`);
  return value as Array<{ [key: string]: JsonValue }>;
}

export interface DynamicStateSpec {
  caseId: string;
  expression: string;
  operations: string[];
}

export const DYNAMIC_STATE_SPECS: DynamicStateSpec[] = [
  {
    caseId: "dynamic-dom",
    expression:
      "({removed:!document.querySelector('#old'),addedText:document.querySelector('#root p')?.textContent,className:document.querySelector('#root p')?.className,state:document.querySelector('#root p')?.dataset.state,color:document.querySelector('#root p')?.style.color})",
    operations: ["load"],
  },
  {
    caseId: "events",
    expression:
      "({result:document.querySelector('#result')?.textContent || '',key:document.querySelector('#keyboard')?.textContent || ''})",
    operations: [
      "focus:#field",
      "input:#field=入力",
      "keydown:#field=Shift",
      "click:button",
      "submit:form",
    ],
  },
  {
    caseId: "async",
    expression: "document.querySelector('#state')?.textContent || ''",
    operations: ["load", "await-timer-and-microtask"],
  },
  {
    caseId: "observers",
    expression: "JSON.parse(document.querySelector('#state')?.textContent || '[]')",
    operations: ["load", "await-mutation-delivery"],
  },
  {
    caseId: "resources",
    expression:
      "({state:document.querySelector('#state')?.textContent,fonts:document.documentElement.dataset.fonts})",
    operations: ["load", "await-resource-and-font"],
  },
];

export function specForDynamicCase(caseId: string): DynamicStateSpec {
  const spec = DYNAMIC_STATE_SPECS.find((candidate) => candidate.caseId === caseId);
  if (!spec) throw new Error(`missing dynamic state spec: ${caseId}`);
  return spec;
}

export function assertExpectedStateMatches(expected: JsonValue, actual: JsonValue): void {
  if (!deepEqual(expected, actual))
    throw new Error(
      `dynamic state mismatch: expected ${JSON.stringify(expected)}, actual ${JSON.stringify(actual)}`,
    );
}

function deepEqual(left: JsonValue, right: JsonValue): boolean {
  if (Object.is(left, right)) return true;
  if (typeof left !== "object" || left === null || typeof right !== "object" || right === null)
    return false;
  if (Array.isArray(left) || Array.isArray(right))
    return (
      Array.isArray(left) &&
      Array.isArray(right) &&
      left.length === right.length &&
      left.every((value, index) => right[index] !== undefined && deepEqual(value, right[index]))
    );
  const leftKeys = Object.keys(left).sort();
  const rightKeys = Object.keys(right).sort();
  return (
    leftKeys.length === rightKeys.length &&
    leftKeys.every(
      (key, index) => key === rightKeys[index] && deepEqual(left[key] ?? null, right[key] ?? null),
    )
  );
}
