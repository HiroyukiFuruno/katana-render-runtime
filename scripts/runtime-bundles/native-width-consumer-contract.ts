const directory = "crates/katana-render-runtime/src/markdown/mermaid_renderer/";

// 実テストとトレースの経路をレビューしたときだけ更新する。
export const NATIVE_WIDTH_CONSUMER_PINS = {
  [`${directory}js_runtime_native_width_helpers.rs`]:
    "0a429cafcd569dc8c1ed993141ef394d0ee10717fe2883f2583af5b7e14232fd",
  [`${directory}js_runtime.rs`]: "6beaa9a5f1b2d52c87d4e34ab55e78ee2a00b57c8a992da3082ab5459300b28a",
  [`${directory}js_runtime_native_width_tests.rs`]:
    "d48f9a98dcd1b4f1dcdd0ad38c2a10580859d0d13d3c3e5dd1110dfb6cae1315",
  [`${directory}js_runtime_native_width_c4_description_tests.rs`]:
    "b5a7e3e5ffb73a22ad6df961f46ad915debb6590d4b7c984a4ceb9e8aa62e6e2",
  [`${directory}js_runtime_native_width_c4_tests.rs`]:
    "28610a5998405703b5a07c6c1023efbf4e3315b2eb5d93b56ad043f8dcef2ead",
  [`${directory}js_runtime_native_width_ishikawa_tests.rs`]:
    "8d818595edc6750bafbead131fef9beea1987a9995a0a2613cabf1b2de3d5a9f",
  [`${directory}js_runtime_native_width_oracle_boundary_tests.rs`]:
    "f4375e4ca17e4d1931efbf21020b7b48bf06079ed9ae00de3ea160d31683731f",
  [`${directory}js_runtime_native_width_trace.js`]:
    "80953163facdebd8d765bb26923f838873043eb673eacf40963c64c0233ffd0e",
  [`${directory}js_runtime_native_width_bbox_trace.js`]:
    "e2aae9c8755431d6a5a0c92c7816b273fc204fb4f8f845cf1eae57574b929d4b",
} as const;
