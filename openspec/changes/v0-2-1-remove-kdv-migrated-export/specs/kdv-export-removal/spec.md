## ADDED Requirements

### Requirement: KDV移譲後にKDRのdocument exportを削除しなければならない

システムは、KDVがHTML / PDF / PNG / JPG exportを提供できる状態になった後、KDR側のdocument export機能を削除しなければならない（MUST）。

#### Scenario: KDV実装完了後にKDR exportを整理する

- **WHEN** KDV v0.1.0がreleaseされ、export APIが利用可能である
- **THEN** KDR libraryのdocument export APIを削除する
- **THEN** KDR CLIのdocument export commandを削除する
- **THEN** KDR docsからdocument export手順を削除する

### Requirement: 図形描画APIは維持されなければならない

システムは、KDV移譲後もMermaid / Draw.io rendering APIを維持しなければならない（MUST）。

#### Scenario: KDVがKDRを利用する

- **WHEN** KDVがMarkdown viewer/export pipeline内でMermaidまたはDraw.ioを描画する
- **THEN** KDRの図形描画APIを呼び出せる
- **THEN** KDRはdocument全体のexport pipelineを持たない
- **THEN** runtime assetとreference scoreの品質ゲートは維持される
