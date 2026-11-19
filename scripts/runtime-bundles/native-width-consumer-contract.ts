const directory = "crates/katana-render-runtime/src/markdown/mermaid_renderer/";

// 実テストとトレースの経路をレビューしたときだけ更新する。
export const NATIVE_WIDTH_CONSUMER_PINS = {
  [`${directory}js_runtime_native_width_helpers.rs`]:
    "a4471da164c4cd7a8958f3dca10f8c15b5a151f71f17cda5847a6387c3fcc96b",
  [`${directory}js_runtime.rs`]: "6beaa9a5f1b2d52c87d4e34ab55e78ee2a00b57c8a992da3082ab5459300b28a",
  [`${directory}js_runtime_native_width_tests.rs`]:
    "c91969f55fdfca06b1444f6f538bddc07c982e7251f5af4b2a2e48f311cfdf60",
  [`${directory}js_runtime_native_width_c4_description_tests.rs`]:
    "e58c77083bbe9865b42e8c6fad3cb07f6c197ecb332686c980ceeabe69a8dbe3",
  [`${directory}js_runtime_native_width_c4_tests.rs`]:
    "28610a5998405703b5a07c6c1023efbf4e3315b2eb5d93b56ad043f8dcef2ead",
  [`${directory}js_runtime_native_width_ishikawa_tests.rs`]:
    "3857d92d2ea01c556b5fa0642b6c2d7123835f5435db5c8942493e0089f6cf47",
  [`${directory}js_runtime_native_width_oracle_boundary_tests.rs`]:
    "d99fb158de06b3d2b91dd20ea8350c902b6480a2076a92c4ec74c079cfa77f43",
  [`${directory}js_runtime_native_width_trace.js`]:
    "80953163facdebd8d765bb26923f838873043eb673eacf40963c64c0233ffd0e",
  [`${directory}js_runtime_native_width_bbox_trace.js`]:
    "e2aae9c8755431d6a5a0c92c7816b273fc204fb4f8f845cf1eae57574b929d4b",
} as const;
