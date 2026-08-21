# mdsee 実装計画

設計の詳細は [design.md](./design.md) を参照する。本書中の `§N` は design.md のセクション番号を指す。

---

## 1. 前提

* 設計の唯一の情報源は [design.md](./design.md) である。
* 実装は `develop` ブランチで行う。Sprintごとに `feat/sprintN-xxx` ブランチを切り、PRは `develop` へ出す。
* リリース時に限り `develop` → `main` へマージし、`main` のタグからartifactをビルドする（§86〜§89）。
* バージョニングは v0.1 → v0.2 → v0.3 → v0.4 のマイルストーン駆動（§2）。マイルストーン間はpatchリリースを許可する。
* 各タスク完了の定義（DoD）は「コード + 単体テスト + `cargo fmt` / `cargo clippy` / `cargo test --workspace` が通っていること」。CI（§85）がすべてのPRで走る。

---

## 2. マイルストーン概要

| マイルストーン | Sprint | 内容 | リリース物 |
|---|---|---|---|
| v0.1 | Sprint 1〜3 | Markdown基本表示・Syntax Highlight・Table・Alert・pipe safety | Homebrew / crates.io |
| v0.2 | Sprint 4〜5 | 画像（Kitty / iTerm2 / Sixel / Unicode fallback・SVG・cache） | 同上 |
| v0.3 | Sprint 6 | Reader（scroll・search・TOC・link・resize） | 同上 |
| v0.4 | Sprint 7 | Rich Document（remote image・Mermaid・Math・watch） | 同上 |

依存関係は常に左へ流れる。Sprint N+1 は Sprint N の成果物の上に載る。

---

## 3. 最初に固定する内部API

実装開始時に、次の5つのinterfaceを先に確定させる（§100）。署名が固まらない間は実装を進めない。

```rust
pub fn load_source(input: InputSource) -> Result<SourceDocument>;

pub fn parse(source: &SourceDocument) -> Result<Document>;

pub fn layout(document: &Document, options: &LayoutOptions) -> Result<LayoutDocument>;

pub fn detect_terminal() -> TerminalCapabilities;

pub fn render(
    document: &LayoutDocument,
    target: &mut dyn Write,
    options: &RenderOptions,
) -> Result<()>;
```

これがすべてのSprintで共通するpipelineの骨格である（§73）。

---

## 4. Sprint詳細

### Sprint 1 — 基本pipeline

**ゴール**: `mdsee README.md` が動く（§96 Sprint 1）。

| ID | タスク | 設計参照 | 主な実装crate |
|---|---|---|---|
| S1-1 | Cargo workspace セットアップ。`crates/` に7 crateの雛形、`rustfmt.toml` / `clippy.toml` / `.gitignore`、workspace dependencies（§3のcrate一覧） | §4, §3 | root |
| S1-2 | 依存方向の実装確認。`cargo tree` で §101 の依存方向（特に `core → terminal` 禁止）を検査するテストを追加 | §101 | root |
| S1-3 | CLIスケルトン。clapで `FILE` / `-V` / `-h` のみ解析。他optionはSprint 2以降で追加。exit code規約（0/1/2）を実装 | §6, §68 | mdsee-cli |
| S1-4 | Input。`InputSource` / `Origin` / `SourceDocument` と `load_source()`（file / stdin / `-`）。`base_dir` 解決 | §9 | mdsee-core |
| S1-5 | Internal AST。`Document` / `Block` / `Inline` / `TextRun` / `Link` / `SourceSpan` / `BlockId`（parse時連番発行） | §10〜§13, §75 | mdsee-core |
| S1-6 | Parser。`MarkdownParser` traitと `ComrakParser`。paragraph / heading / emphasis / strong / strike / inline code / softbreak / hardbreak / text を変換。HTMLは§15の方針（inlineはtag除去、blockはtext fallback） | §14, §15 | mdsee-core |
| S1-7 | Layout基盤。`LayoutDocument` / `LayoutBlock` / `TextBlock` / `LayoutLine` / `LayoutSpan` / `SemanticStyle` / `LayoutContext`。`content_width` 計算（margin 2、max_width 100） | §16〜§19, §21, §22 | mdsee-layout |
| S1-8 | Paragraph wrapping。`unicode-width` / `unicode-segmentation` 導入。word boundary（英語）+ grapheme boundary（CJK）+ URL等のsoft break。`str.len()` を表示幅に使わない | §20, §23 | mdsee-layout |
| S1-9 | Heading layout。H1は `━` 下線、H2は `─` 下線、H3以下はそのまま | §24 | mdsee-layout |
| S1-10 | Inline style layout。strong / emphasis / strike / inline code を `SemanticStyle` 付きspanへ | §12, §19 | mdsee-layout |
| S1-11 | ANSI renderer。`SemanticStyle` → ANSI色（TrueColor / 256 / 16の降格を含む）。§100の `render()` 署名を実装 | §19, §35 | mdsee-render |
| S1-12 | pipeline接続。args → load_source → parse → layout → render → stdout | §73 | mdsee-cli |

**DoD**:

```bash
cargo run -- README.md        # heading / paragraph / inline style がANSI色で表示される
cargo test --workspace        # parser・wrapping の単体テストが通る
```

日本語・絵文字のwrapテスト（`日本語` / `🙂` / `👨‍💻` / `é`）をこのSprintで入れ始める（§20, §84）。

---

### Sprint 2 — ブロック要素とTTY対応

**ゴール**: 通常のREADMEがすべてのblock要素として読める。pipe safetyが効く（§96 Sprint 2）。

| ID | タスク | 設計参照 | 主な実装crate |
|---|---|---|---|
| S2-1 | List。bullet（`•`）/ ordered / task（`☐` `☑`、Unicode不可なら `[ ]` `[x]`）。ネストとindent | §25 | mdsee-layout |
| S2-2 | Blockquote。`│` prefix、nested quote対応 | §26 | mdsee-layout |
| S2-3 | HorizontalRule | §11, §17 | mdsee-layout |
| S2-4 | LinkとOSC 8。`TerminalCapabilities.osc8` 判定、plain fallbackは `text <URL>` | §33, §34 | mdsee-render / mdsee-terminal |
| S2-5 | TerminalCapabilities基本。TTY判定、columns / rows、`ColorLevel`。環境変数（`NO_COLOR` / `COLORTERM` / `TERM` / `TERM_PROGRAM`）。passive detectionのみ | §34, §35, §37 | mdsee-terminal |
| S2-6 | 自動モード判定。`OutputMode`（Print / Reader / Plain）。stdout非TTY → Plain | §8 | mdsee-cli |
| S2-7 | Pipe behavior。非TTY時にANSI OFF / OSC8 OFF / images OFF / reader OFF | §71 | mdsee-cli / mdsee-render |
| S2-8 | `--plain` / `--force-color` / `--no-color` / `--width` / `--max-width` / `--theme` を追加。`NO_COLOR` は `--force-color` でのみ上書き | §7, §72 | mdsee-cli |
| S2-9 | Theme。`Theme` 構造と auto / dark / light。`SemanticStyle` → `Theme` → ANSIの変換経路を固定 | §19, §61, §62 | mdsee-render |
| S2-10 | Config。config.toml読み込みとpriority（CLI > env > file > defaults）。§64の全キーを定義（未使用キーは後のSprintで使う） | §63〜§65 | mdsee-cli |

**DoD**:

```bash
cargo run -- README.md > out.txt   # out.txt にANSI escapeが1つも入らない
cargo run -- -p README.md          # plain表示
cargo test --workspace
```

---

### Sprint 3 — Code / Table / Alert（v0.1）

**ゴール**: v0.1 Acceptance Criteria（§97）をすべて通す。

| ID | タスク | 設計参照 | 主な実装crate |
|---|---|---|---|
| S3-1 | CodeBlock layout。`╭─ 言語 ─` 枠。横スクロールはReader待ち、折返しはしない | §28 | mdsee-layout |
| S3-2 | Syntax highlight。`SyntaxHighlighter` traitと `SyntectHighlighter`。rendererはsyntectに直接依存させない | §29 | mdsee-render |
| S3-3 | Table。`TableLayoutEngine`。min / preferred / assignedのcolumn計算、余裕の大きいcolumnから削る縮小、alignment（left / center / right） | §30〜§32 | mdsee-layout |
| S3-4 | GFM Alert。`╭─ WARNING ─` 枠とAlert系style | §27 | mdsee-layout / mdsee-render |
| S3-5 | Snapshot test基盤。fixtures（headings / tables / japanese / emoji / code / alerts / complete）をANSI normalizeして比較 | §81 | tests |
| S3-6 | 日本語・Unicode edge caseテスト一式（§84の7項目） | §84 | tests |
| S3-7 | Integration test。バイナリ実行してexit status / stdout / stderr を検証 | §83 | tests/integration |
| S3-8 | CI整備。Ubuntu / macOS / Windowsでfmt / clippy -D warnings / test / build | §85 | .github |
| S3-9 | Release整備。release profile（§91）、feature flags（§92）、release.yml（§86, §87）、Homebrew tap（§88, §89）、crates.io公開確認（§90） | §86〜§92 | .github / root |

**DoD（v0.1 acceptance）**:

```bash
mdsee README.md
cat README.md | mdsee
mdsee japanese.md
mdsee table.md
mdsee code.md
mdsee README.md > out.txt    # ANSI escape が含まれないこと
```

---

### Sprint 4 — Terminal Graphics基盤

**ゴール**: Kitty / iTerm2 系terminalでMarkdown中にローカル画像が出る。

| ID | タスク | 設計参照 | 主な実装crate |
|---|---|---|---|
| S4-1 | TerminalCapabilities拡張。`graphics` / `cell_pixels` / active query（Readerと `--detect-terminal` のみ） | §34, §37 | mdsee-terminal |
| S4-2 | `GraphicsProtocol` と自動選択（Kitty → iTerm2 → Sixel → Unicode → None）、terminal別override table | §36 | mdsee-terminal |
| S4-3 | ImageSource解決。`ImageBlock` をBlockへ昇格、`base_dir` からの相対解決 | §9, §12, §38 | mdsee-core |
| S4-4 | Image pipeline。`ImageLoader` / `DecodedImage` / `ImageSizing` / `RasterImage`。PNG / JPEG / GIF静止画（first frameのみ）/ WebP。aspect ratio維持 | §39, §41, §44 | mdsee-image |
| S4-5 | Pixel geometry。cell pixel取得時はpixel換算、未取得時はprotocol側cell単位resize | §42 | mdsee-image |
| S4-6 | `ImageBackend` trait | §40 | mdsee-image |
| S4-7 | Kitty backend。transmit → image id → placement を分離。再転送防止 | §45 | mdsee-image |
| S4-8 | iTerm2 backend。OSC 1337 + base64、cellベースサイズ指定 | §46 | mdsee-image |
| S4-9 | `ImageLayout` の配置と `--images` / `--no-images` / `--graphics` option | §17, §7 | mdsee-layout / mdsee-cli |
| S4-10 | Terminal snapshot test。OSC 8 / Kitty command / iTerm2 command のbyte列検証 | §82 | tests |

**DoD**: 手元のKitty / Ghostty / iTerm2 / WezTerm で `mdsee docs/image-sample.md` のPNGが表示される。画像無効環境では `[image unavailable: ...]` 等にfall backしてexit code 0（§67, §68）。

---

### Sprint 5 — Sixel / SVG / fallback / cache（v0.2）

**ゴール**: v0.2 Acceptance Criteria（§98）を通す。

| ID | タスク | 設計参照 | 主な実装crate |
|---|---|---|---|
| S5-1 | Sixel backend。encoder crateを `SixelEncoder` adapterの裏に隠す。cell pixelが不明時は推定値fallback | §42, §47 | mdsee-image |
| S5-2 | SVG rasterize。usvg parse → resvg render → RGBA。外部URL取得禁止 | §43 | mdsee-image |
| S5-3 | Unicode half-block fallback。`▀` + TrueColor、2px/cell | §48 | mdsee-image |
| S5-4 | Image cache。XDG cache dir / macOS標準dir。SHA-256キー（source bytes + size + protocol + renderer version） | §50 | mdsee-image |
| S5-5 | 画像系エラーのgraceful degradation。失敗してもdocument全体を失敗させない | §67 | mdsee-image / mdsee-render |

**DoD（v0.2 acceptance）**: Ghostty / Kitty / iTerm2 / WezTerm / foot で画像が表示される。各terminalでcapabilitiesに応じた正しいbackendが選択されること（全terminalで同一protocolを使わない）。

---

### Sprint 6 — Reader（v0.3）

**ゴール**: interactive readerが実用になる。

| ID | タスク | 設計参照 | 主な実装crate |
|---|---|---|---|
| S6-1 | ratatui基盤。`ReaderState` / `Viewport`、`-r` option、alt screenへの切り替えと復帰 | §51, §52, §7 | mdsee-reader |
| S6-2 | Scroll。j / k / Ctrl-d / Ctrl-u / Space / b / g / G、h / l で水平scroll | §53 | mdsee-reader |
| S6-3 | Resize。layout invalidation → relayout → `BlockId + offset` で論理位置保持 | §52 | mdsee-reader |
| S6-4 | Search。rendered textへの `SearchIndex`、`/` `n` `N`、結果のlayout line変換とhighlight | §54 | mdsee-reader |
| S6-5 | TOC。headingから `TocEntry` 生成、`t` で開く、jump | §55 | mdsee-reader |
| S6-6 | Link navigator。viewport内link収集、`o` でselector、1〜9は即open、`UrlOpener`（open / xdg-open / start） | §56, §57 | mdsee-reader |
| S6-7 | Mouse support（config `reader.mouse`） | §64 | mdsee-reader |
| S6-8 | Reader first paint < 100 ms の計測と回帰防止 | §77 | mdsee-reader |

**DoD**: `mdsee -r README.md` でscroll / search / TOC / link open / resize がすべて動作する。`q` で端末状態を完全に復元する。

---

### Sprint 7 — Rich Document（v0.4）

**ゴール**: remote image・Mermaid・Math・watch。

| ID | タスク | 設計参照 | 主な実装crate |
|---|---|---|---|
| S7-1 | Remote image。blocking reqwest（§74の方針）。redirect 3 / timeout 5s / 20MB上限。private IP・localhost拒否とDNS rebinding対策（解決後IP検査）。`--offline` | §49, §74 | mdsee-image |
| S7-2 | Watch mode。`notify` + 100ms debounce → reload → parse → layout → render（full re-render）。`-w` はReaderで起動 | §58 | mdsee-reader / mdsee-cli |
| S7-3 | Mermaid。外部 `mmdc`（PATH確認）→ SVG → image pipeline。不在時は `[mermaid diagram — install mermaid-cli to render]` | §59 | mdsee-image |
| S7-4 | Block math。外部renderer（Typst or KaTeX）→ SVG → image pipeline。inline mathはやらない | §60 | mdsee-image |
| S7-5 | feature flagsの最終確認（`mermaid` / `math` / `network` は §92 の依存関係どおり） | §92 | root |

**DoD**: それぞれの外部tool不在時でもMarkdown表示自体は成功し、exit code 0（§67）。

---

## 5. Acceptance Criteria チェックリスト

### v0.1（§97）

- [ ] `mdsee README.md` が装飾付きで表示される
- [ ] `cat README.md | mdsee` が動く
- [ ] `mdsee japanese.md` で日本語wrapが崩れない
- [ ] `mdsee table.md` でtableがcontent_width内に収まる
- [ ] `mdsee code.md` でsyntax highlightされる
- [ ] `mdsee README.md > out.txt` のout.txtにANSI escapeが入らない

### v0.2（§98）

- [ ] Ghostty（Kitty graphics）で画像表示
- [ ] Kitty（Kitty graphics）で画像表示
- [ ] iTerm2（inline image）で画像表示
- [ ] WezTerm（対応protocol）で画像表示
- [ ] foot（Sixel）で画像表示
- [ ] 非対応terminalでUnicode half-block fallback
- [ ] TerminalCapabilitiesによるbackend選択がterminalごとに正しい
- [ ] 画像なしの状態に比べて起動が顕著に遅くならない（§77）

---

## 6. 性能・メモリの回帰防止

§77 / §78 の目標を、v0.1の時点でbenchmark harness（`cargo bench` 相当でよい）として用意する。

```text
10 KB    < 15 ms
100 KB   < 50 ms
1 MB     < 250 ms
Reader first paint < 100 ms
通常README < 30 MB RSS
```

benchmarkはCIでは時間の都合でnightly / 手動実行とし、劣化が見えたらSprintのDoDに組み込む。

---

## 7. リスクと対策

| リスク | 影響Sprint | 対策 |
|---|---|---|
| comrakのASTとInternal ASTのギャップが想定より大きい | S1 | 変換層を薄く保ち、`MarkdownParser` trait（§14）でparser差し替え可能にしておく |
| Unicode wrapping（grapheme × width × soft break）の複雑さ | S1〜S3 | Sprint 1から§84のテストを書き続ける。wrapは独立moduleにして純関数化する |
| Sixel encoder crateの品質・保守性 | S5 | §47のとおりadapterで隔離し、独自codec（§99）は書かない。必要ならencoder差し替え |
| Kitty graphicsのimage id管理と再転送 | S4〜S6 | §45のtransmit / placement分離をSprint 4の最初から強制する |
| Terminal実機検証の属人化 | S4〜S5 | §98の5端末を手動チェックリスト化し、検証結果をPRに記録する |
| binary sizeの肥大化 | 常時 | §91のrelease profileと§92のfeature flagsをSprint 3で導入し、artifactサイズをreleaseごとに記録する |
| crates.ioの `mdsee` 名が取れない | S3 | 公開前に名前確認。取れない場合は配布をHomebrew主体にしてcrate名を `mdsee-cli` 等へ分離する |
| WindowsでのOSC 8 / 画像非対応 | S3〜S5 | capabilities判定で機能を落とす（§34）。CIは§85の3 OSで回す |
| 外部tool（mmdc / KaTeX / Typst）の環境差異 | S7 | PATH検出とgraceful degradation（§59, §60, §67）をDoDに含める |

---

## 8. スコープ外の再確認

実装中に誘惑されやすいが、§99のとおりv0.xではやらない。

```text
Markdown Editor / editing
Browser preview
HTML完全互換 / CSS
Plugin system
GitHub API
LLM integration
PDF
Presentation mode
Full Mermaid implementation
Custom Sixel codec
```

追加提案があった場合は設計書（design.md）の改訂を先に行い、本計画のSprintへ明示的に組み込んでから着手する。
