interface KrrElement {
  readonly __krrNodeId: string;
}

type KrrLoadHandler = (event: unknown) => unknown;
type KrrWeakGet = <Value>(receiver: WeakMap<object, Value>, key: object) => Value | undefined;
type KrrWeakSet = <Value>(
  receiver: WeakMap<object, Value>,
  key: object,
  value: Value,
) => WeakMap<object, Value>;
type KrrMapGet = <Key, Value>(receiver: Map<Key, Value>, key: Key) => Value | undefined;
type KrrMapSet = <Key, Value>(receiver: Map<Key, Value>, key: Key, value: Value) => Map<Key, Value>;
type KrrMapDelete = <Key, Value>(receiver: Map<Key, Value>, key: Key) => boolean;
type KrrSetDelete = <Value>(receiver: Set<Value>, value: Value) => boolean;

interface KrrParserNativeDom {
  (operation: "querySelector", selector: string): string | null | undefined;
  (operation: "getAttribute", nodeId: string | undefined, name: string): string | null | undefined;
  (operation: "setAttribute", nodeId: string | undefined, name: string, value: string): undefined;
}

// Rust bridge は NodeId を文字列化し、失敗時も正常な setAttribute も undefined を返す。
declare var __krrNativeDom: KrrParserNativeDom;
declare var __krrElement: (nodeId: string | null | undefined) => KrrElement | null;
declare var __krrEventHandlers: WeakMap<object, Map<string, KrrLoadHandler>>;
declare var __krrLifecyclePropertyOverrides: WeakMap<object, Set<string>>;
declare var __krrElements: Map<string, KrrElement>;
declare var window: object;
