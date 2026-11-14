import fs from "node:fs";
import path from "node:path";
import type { OfficialReferenceMode } from "./official-reference-mode";
import type { ReferencePair } from "./reference_pair_repository";

interface ReferenceCompareScoreRow {
  slug: string;
  score: number;
  scoreSemantics?: string;
  rgbScore?: number;
  paintProjectedReference?: {
    fullCanvasRmseScore: number;
    fullCanvasMaeScore: number;
    contentRmseScore: number;
    contentMaeScore: number;
    structuralIntegrityAccepted: boolean;
  };
  minScore: number;
  canvasRmseScore: number;
  canvasMaeScore: number;
  contentRmseScore: number;
  contentMaeScore: number;
  passed: boolean;
  exceptionReason: string;
}

export interface ReferenceRunContext {
  mode: OfficialReferenceMode;
  scope: "formal" | "diagnostic";
  expectedSlugCount: number;
  emptySlugCount: number;
  scoreThreshold: number;
  formalAccepted: false;
  independentGold: false;
  scoreSemantics: "rgb-v1" | "rgb-v1+paint-projected-reference-v1";
  paintProjectedReferenceEvidence?: { path: string; sha256: string };
}

export class ReferenceCompareReport {
  constructor(
    private outputDir: string,
    private labels: ReferenceCompareReportLabels = ReferenceCompareReportLabels.mermaid(),
  ) {}

  write(
    pairs: ReferencePair[],
    scores: ReferenceCompareScoreRow[],
    contactSheet: string,
    context?: ReferenceRunContext,
  ) {
    const contactSheetPath = path.relative(this.outputDir, contactSheet);
    fs.writeFileSync(
      path.join(this.outputDir, "scores.json"),
      JSON.stringify(scores, null, 2),
      "utf8",
    );
    fs.writeFileSync(
      path.join(this.outputDir, "README.md"),
      this.markdown(pairs, scores, contactSheetPath),
      "utf8",
    );
    if (context !== undefined) {
      fs.writeFileSync(
        path.join(this.outputDir, "reference-context.json"),
        `${JSON.stringify(context, null, 2)}\n`,
        "utf8",
      );
    }
  }

  private markdown(
    pairs: ReferencePair[],
    scores: ReferenceCompareScoreRow[],
    contactSheetPath: string,
  ): string {
    return [
      this.labels.title,
      "",
      this.labels.description,
      "",
      "## 一致度スコア",
      "",
      "| 図 | 採点方式 | 総合 | RGB総合 | 射影全体RMSE | 射影全体MAE | 射影内容RMSE | 射影内容MAE | 構造 | 下限 | 全体RMSE | 全体MAE | 内容RMSE | 内容MAE | 判定 | 例外理由 |",
      "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | --- | --- |",
      ...scores.map((score) => this.scoreRow(score)),
      "",
      `![全体比較](${contactSheetPath})`,
      "",
      ...pairs.flatMap((pair) => this.pairSection(pair)),
      "",
    ].join("\n");
  }

  private scoreRow(score: ReferenceCompareScoreRow): string {
    const columns = [
      score.slug,
      score.scoreSemantics ?? "rgb-v1",
      score.score.toFixed(2),
      (score.rgbScore ?? score.score).toFixed(2),
      score.paintProjectedReference?.fullCanvasRmseScore.toFixed(2) ?? "—",
      score.paintProjectedReference?.fullCanvasMaeScore.toFixed(2) ?? "—",
      score.paintProjectedReference?.contentRmseScore.toFixed(2) ?? "—",
      score.paintProjectedReference?.contentMaeScore.toFixed(2) ?? "—",
      score.paintProjectedReference?.structuralIntegrityAccepted
        ? "一致"
        : score.paintProjectedReference
          ? "不一致"
          : "—",
      score.minScore.toFixed(2),
      score.canvasRmseScore.toFixed(2),
      score.canvasMaeScore.toFixed(2),
      score.contentRmseScore.toFixed(2),
      score.contentMaeScore.toFixed(2),
      score.passed ? "OK" : "NG",
      score.exceptionReason,
    ];
    return `| ${columns.join(" | ")} |`;
  }

  private pairSection(pair: ReferencePair): string[] {
    return [`## ${pair.slug}`, "", `![${pair.slug}](pairs/${pair.slug}.png)`, ""];
  }
}

export class ReferenceCompareReportLabels {
  static mermaid(): ReferenceCompareReportLabels {
    return new ReferenceCompareReportLabels(
      "# Mermaid 公式参照比較",
      "左が公式 Mermaid.js の SVG、右が KatanA 生成 SVG です。どちらも同じブラウザー条件で画像化（rasterize）して比較しています。",
    );
  }

  static drawio(): ReferenceCompareReportLabels {
    return new ReferenceCompareReportLabels(
      "# Draw.io 公式参照比較",
      "左が公式 Draw.io JavaScript の実ブラウザー描画、右が KatanA preview のスクリーンショットです。",
    );
  }

  static plantuml(): ReferenceCompareReportLabels {
    return new ReferenceCompareReportLabels(
      "# PlantUML 公式参照比較",
      "左が公式 PlantUML の SVG、右が KDR 生成 SVG です。どちらも同じブラウザー条件で画像化（rasterize）して比較しています。",
    );
  }

  constructor(
    public title: string,
    public description: string,
  ) {}
}
