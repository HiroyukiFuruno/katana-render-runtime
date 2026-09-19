## 1. Layout ownership and clip metadata

- [ ] 1.1 Add a stable element-owner identity and viewport sentinel to containing-block and fragment layout outputs.
- [ ] 1.2 Collect clipping ancestor identities and effective clip rectangles for element fragments without changing SVG clipping output.
- [ ] 1.3 Add unit coverage for viewport roots, descendant roots, containing-block escapes, and nested overflow clips.

## 2. Runtime bridge contract

- [ ] 2.1 Extend the HTML runtime layout snapshot with owner, containing-block, and clip metadata for each observed fragment.
- [ ] 2.2 Reject missing, duplicate, or inconsistent metadata at the bridge boundary.
- [ ] 2.3 Add Rust-to-runtime contract tests for complete and malformed metadata.

## 3. IntersectionObserver evaluation

- [ ] 3.1 Require a registered root to be an ancestor of the target's event path before computing intersection.
- [ ] 3.2 Report an empty zero-ratio intersection when the target escapes the root containing block.
- [ ] 3.3 Intersect root bounds and every clipping ancestor rectangle before threshold delivery.
- [ ] 3.4 Cover ancestor transforms, nested clips, and fragmentable inline targets in browser-backed tests.

## 4. Validation and issue completion

- [ ] 4.1 Run the targeted runtime, DOM, AST, lint, and full check gates.
- [ ] 4.2 Update Issue #78 with the implementation evidence and close it only after all acceptance criteria pass.
