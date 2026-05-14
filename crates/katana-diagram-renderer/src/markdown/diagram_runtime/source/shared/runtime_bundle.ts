export type RuntimeBundleName = "mermaid" | "drawio" | "zenuml";

export type RuntimeFragmentTransform =
  | "verbatim"
  | "drawio-runtime-adapter-without-installer"
  | "drawio-render-without-auto-call";

export interface RuntimeFragment {
  readonly label: string;
  readonly sourcePath: string;
  readonly transform: RuntimeFragmentTransform;
}

export class RuntimeBundleDefinition {
  constructor(
    readonly name: RuntimeBundleName,
    readonly outputFile: string,
    readonly fragments: readonly RuntimeFragment[],
    readonly entrypoint: string,
  ) {}
}

export class RuntimeFragmentFactory {
  static verbatim(label: string, sourcePath: string): RuntimeFragment {
    return {
      label,
      sourcePath,
      transform: "verbatim",
    };
  }

  static transformed(
    label: string,
    sourcePath: string,
    transform: RuntimeFragmentTransform,
  ): RuntimeFragment {
    return {
      label,
      sourcePath,
      transform,
    };
  }
}
