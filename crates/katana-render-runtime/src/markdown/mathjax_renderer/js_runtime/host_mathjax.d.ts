interface MathJaxRuntimeRequest {
  source: string;
  display: boolean;
  text: string;
  darkMode: boolean;
}
declare var katanaRunMathJaxRuntime: (request: MathJaxRuntimeRequest) => string;
