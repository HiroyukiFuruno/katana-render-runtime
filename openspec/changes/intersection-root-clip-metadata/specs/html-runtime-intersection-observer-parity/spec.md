## ADDED Requirements

### Requirement: Element root observer reflects layout containing-block ownership
The runtime SHALL exclude a fixed or absolute target from an Element-root `IntersectionObserver` when the target's resolved containing block is outside that root's DOM event path. It MUST report `isIntersecting=false`, `intersectionRatio=0`, and an empty intersection rectangle.

#### Scenario: Fixed descendant uses viewport containing block
- **WHEN** an Element-root observer observes its fixed-position DOM descendant whose containing block is the viewport
- **THEN** the observer reports the target as non-intersecting with ratio zero and an empty rectangle

#### Scenario: Absolute descendant uses a root-internal containing block
- **WHEN** an Element-root observer observes its absolute-position descendant whose containing block is within the root event path
- **THEN** the observer calculates intersection from the target and root rectangles normally

### Requirement: Element root observer applies ancestor overflow clips
The runtime SHALL intersect a target rectangle with every layout-provided clipping ancestor that is inside the Element root event path. It MUST not treat an `overflow: visible` ancestor as a clipping ancestor.

#### Scenario: Partially clipped target
- **WHEN** a target is partially outside an ancestor with overflow clipping
- **THEN** the observer reports the clipped intersection rectangle and ratio

#### Scenario: Fully clipped target
- **WHEN** clipping ancestors remove the entire target rectangle
- **THEN** the observer reports non-intersection, ratio zero, and an empty rectangle

### Requirement: Incomplete layout observer metadata fails closed
The runtime SHALL treat missing or inconsistent containing-block or clip metadata as non-intersection for an Element-root observer.

#### Scenario: Target lacks containing-block metadata
- **WHEN** an Element-root observer evaluates a positioned target with missing containing-block metadata
- **THEN** the observer reports non-intersection without falling back to viewport ownership
