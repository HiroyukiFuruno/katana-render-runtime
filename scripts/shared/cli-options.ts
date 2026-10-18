export function getOptionValue(argv: readonly string[], name: string, fallback: string): string {
  const index = argv.indexOf(name);
  return index >= 0 ? (argv.at(index + 1) ?? fallback) : fallback;
}

export function printHelpIfRequested(argv: readonly string[], usage: string): boolean {
  if (!argv.includes("--help")) return false;
  console.log(usage);
  return true;
}
