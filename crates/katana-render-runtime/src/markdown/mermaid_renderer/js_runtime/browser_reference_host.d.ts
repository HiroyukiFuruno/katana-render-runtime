// 公式比較のscript注入が提供する値だけを宣言し、製品DOM型へ拡張しない。
declare var __katanaMermaidDiagramType: string | undefined;
declare const KATANA_ASCII_TEXT_WIDTHS: {
  readonly " ": number;
  readonly [character: string]: number | undefined;
};
declare const KATANA_ASCII_KERNING_PAIRS: Readonly<Record<string, number | undefined>>;
