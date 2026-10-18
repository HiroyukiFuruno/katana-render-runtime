# Public font-probe fixtures

These public fixtures exercise isolated font and raster paths in normal-lane tests. They do not define rendering correctness baselines.

## Recursive font

`recursive-latin-crsv-normal.ttf` is copied unchanged from the existing SFNT derived from Mermaid Live's public Recursive WOFF2 asset. Its SHA-256 is `f0b8414e5c903579bfd9dbca230a49f9063d92c6d622c6700f121a50a61e06ba` (133716 bytes). The recorded source WOFF2 SHA-256 is `f1bbb8570693c8b05a48444d5eec33037965f9c3181f9c07f8ae83fa56a0ca71`; the existing conversion record says fontTools `TTFont` with `flavor=None`, save, and reopen. This preparation does not regenerate or modify the SFNT.

The font contains `Copyright 2019 The Recursive Project Authors (https://github.com/arrowtype/recursive)`. The complete official SIL Open Font License 1.1 text is included at `licenses/Recursive-OFL-1.1.txt`; it retains the upstream `Copyright 2020 The Recursive Project Authors` notice. The license was fetched from the official Arrowtype repository at commit `6d491202cea5cf6a493ef710cbef2527b9b08939`, from [the pinned OFL.txt](https://raw.githubusercontent.com/arrowtype/recursive/6d491202cea5cf6a493ef710cbef2527b9b08939/OFL.txt). Its SHA-256 is `f9f539cf7549bd417159dbdb9c400943a5b60a7366c2c6fbde9f095173d82479`.

## Native generated SVG

`native-class-ja-probe.svg` is an unchanged copy of an existing KRR-native generated SVG from the public Mermaid class-diagram sample and its pinned public theme. SHA-256: `8e4a91dd90ea7f9ebb0607aa1ff592ed9d548def31691bb79aed5c63701dfc12` (35648 bytes).

The filename deliberately identifies this as a native-produced diagnostic input. It may be used to exercise the font/raster path; it is not an independent raw-JavaScript baseline, a Mermaid correctness oracle, or a Mermaid Live appearance oracle. The recorded SVG has no embedded font binaries. Mermaid 12.1 is attributed as the generator; this attribution does not assert that the Mermaid source license automatically governs the emitted SVG.
