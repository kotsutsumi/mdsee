# mdsee 実装詳細設計書

本書は `mdsee` の設計詳細を定義する。実装の手順・タスク分解は [implementation-plan.md](./implementation-plan.md) を参照すること。

元資料のセクション番号（§1〜§104）はそのまま維持する。実装計画・コード・Issue から本書を参照する場合はこの番号を使う。

---

## Part I — 概要

### 1. 目的

`mdsee` は、Markdownをターミナル上で高品質に表示するCLIビューアである。

主用途は次のとおり。

```bash
mdsee README.md
cat README.md | mdsee
git show HEAD:README.md | mdsee
gh pr view 123 --json body --jq .body | mdsee
```

単なるANSI装飾ツールではなく、以下を統合する。

* CommonMark / GFM
* Syntax Highlight
* Table
* OSC 8 Hyperlink
* ローカル画像
* リモート画像
* Kitty Graphics Protocol
* iTerm2 Inline Image Protocol
* Sixel
* Unicode fallback
* Mermaid
* 数式
* Reader / Pager
* 検索
* TOC
* Watch
* stdout pipe safety

設計上の最重要原則は、

> Markdown本文はテキストとして描画し、画像・図のみTerminal Graphics Protocolを利用する。

ことである。

Markdown全体を画像化しない。

### 2. スコープ

#### 2.1 v0.1

最初に完成させる範囲。

* Markdown parsing
* Heading
* Paragraph
* Bold / Italic / Strike
* Inline Code
* Code Block
* Syntax Highlight
* List
* Task List
* Blockquote
* Horizontal Rule
* Table
* Link
* GFM Alert
* stdin
* terminal width
* ANSI / TrueColor
* TTY判定
* plain fallback
* theme
* Homebrew配布

#### 2.2 v0.2

画像。

* PNG
* JPEG
* GIF静止画
* WebP
* SVG
* Kitty
* iTerm2
* Sixel
* Unicode half-block fallback
* Terminal capability detection
* image cache

#### 2.3 v0.3

Reader。

* scrolling
* search
* TOC
* link selector
* resize
* mouse
* horizontal scrolling

#### 2.4 v0.4

Rich Document。

* Mermaid
* Math
* remote images
* watch mode

cacheは §76 のとおり、v0.xではimage cacheのみ実装する。

### 3. 技術スタック

言語はRust。

Rust workspaceとして構築する。

主要候補：

```text
CLI
  clap

Markdown
  comrak

Terminal
  crossterm

TUI
  ratatui

Syntax Highlight
  syntect

Unicode width
  unicode-width
  unicode-segmentation

Images
  image

SVG
  resvg / usvg

HTTP
  reqwest

File watch
  notify

Serialization
  serde
  toml

Error
  thiserror
  anyhow

Directories
  directories

Hash
  sha2
```

Markdown parserは、今回は `pulldown-cmark` より `comrak` を第一候補とする。

理由はASTを直接取得しやすく、

```text
Markdown
 ↓
Comrak AST
 ↓
mdsee AST
```

への変換が実装しやすいため。

---

## Part II — リポジトリとcrate設計

### 4. Repository構成

```text
mdsee/
├── Cargo.toml
├── Cargo.lock
├── rustfmt.toml
├── clippy.toml
├── README.md
├── LICENSE
├── .github/
│   └── workflows/
│       ├── ci.yml
│       └── release.yml
│
├── crates/
│   ├── mdsee-core/
│   ├── mdsee-layout/
│   ├── mdsee-render/
│   ├── mdsee-terminal/
│   ├── mdsee-image/
│   ├── mdsee-reader/
│   └── mdsee-cli/
│
└── tests/
    ├── fixtures/
    ├── snapshots/
    └── integration/
```

### 5. crate責務

#### mdsee-core

MarkdownをDocument ASTへ変換する。

担当：

* Markdown parser
* Internal AST
* source location
* link resolution
* image source resolution
* document metadata

Terminalには依存しない。

#### mdsee-layout

Document ASTを表示可能なレイアウトへ変換する。

担当：

* line wrapping
* paragraph layout
* table layout
* list indentation
* code block sizing
* image placement
* responsive layout

Terminal escape sequenceは生成しない。

#### mdsee-render

Layout Treeから文字列を生成する。

担当：

* ANSI
* styles
* borders
* syntax highlighting
* plain text rendering
* hyperlink OSC 8

#### mdsee-terminal

Terminal capabilityを扱う。

担当：

* TTY
* width / height
* truecolor
* OSC 8
* Kitty
* iTerm2
* Sixel
* pixel geometry

#### mdsee-image

画像をTerminalへ出力する。

担当：

* load
* decode
* resize
* SVG rasterize
* protocol backend
* image cache

#### mdsee-reader

interactive reader。

担当：

* scroll
* viewport
* TOC
* search
* link navigation
* mouse
* resize

#### mdsee-cli

実行ファイル。

責務は最小限。

```text
args
 ↓
config
 ↓
input
 ↓
pipeline
```

だけを担当する。

---

## Part III — CLI仕様

### 6. CLI

基本：

```bash
mdsee [OPTIONS] [FILE]
```

FILE省略時：

```text
stdinがpipe
    → stdinを読む

stdinもTTY
    → help表示
```

例：

```bash
mdsee README.md
mdsee -
cat README.md | mdsee
```

### 7. CLI Options

```text
mdsee [FILE]

Display:
  -p, --plain
  -r, --reader
  -w, --watch

Layout:
      --width <N>
      --max-width <N>

Theme:
      --theme <NAME>

Images:
      --images
      --no-images
      --graphics <auto|kitty|iterm|sixel|unicode|none>

Network:
      --offline

Terminal:
      --detect-terminal
      --force-color
      --no-color

Debug:
      --debug
      --dump-ast
      --dump-layout

General:
  -V, --version
  -h, --help
```

### 8. 自動モード判定

起動時：

```rust
enum OutputMode {
    Print,
    Reader,
    Plain,
}
```

判定：

```text
stdout != TTY
    → Plain

--plain
    → Plain

--reader
    → Reader

--watch
    → Reader

otherwise
    → Print
```

初期版では自動Reader移行を行わない。

つまり、

```bash
mdsee README.md
```

は常に出力して終了。

これはUnix CLIとして自然。

将来的に、

```text
pager = auto
```

を導入してもよい。

### 9. Input設計

```rust
pub enum InputSource {
    File(PathBuf),
    Stdin,
}
```

読み込み結果：

```rust
pub struct SourceDocument {
    pub content: String,
    pub origin: Origin,
}
```

```rust
pub enum Origin {
    File {
        path: PathBuf,
        base_dir: PathBuf,
    },
    Stdin {
        cwd: PathBuf,
    },
}
```

`base_dir` が重要。

Markdown中の、

```markdown
![foo](images/foo.png)
```

を正しく解決するため。

---

## Part IV — Markdown Core

### 10. Internal AST

Parser固有ASTをRendererへ漏らさない。

```rust
pub struct Document {
    pub blocks: Vec<Block>,
    pub metadata: DocumentMetadata,
}
```

### 11. Block

```rust
pub enum Block {
    Heading(Heading),
    Paragraph(Paragraph),
    CodeBlock(CodeBlock),
    BlockQuote(BlockQuote),
    List(List),
    Table(Table),
    Image(ImageBlock),
    HorizontalRule,
    Alert(Alert),
    Math(MathBlock),
}
```

### 12. Inline

```rust
pub enum Inline {
    Text(TextRun),
    Emphasis(Vec<Inline>),
    Strong(Vec<Inline>),
    Strike(Vec<Inline>),
    Code(String),
    Link(Link),
    SoftBreak,
    HardBreak,
}
```

画像はlayout都合からBlock化してもよい。

Inline imageはMVPではBlock扱いに昇格する。

### 13. SourceSpan

全Nodeに可能な限りsource positionを持たせる。

```rust
pub struct SourceSpan {
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}
```

将来的な、

```text
rendered line
 ↓
source Markdown
```

へのmappingに利用。

### 14. Parser

インターフェース：

```rust
pub trait MarkdownParser {
    fn parse(
        &self,
        source: &SourceDocument
    ) -> Result<Document>;
}
```

実装：

```rust
pub struct ComrakParser;
```

Comrak ASTを再帰的にInternal ASTへ変換する。

### 15. HTML

Markdown内HTMLは安全性と表示互換性を考えて、

v0.xでは、

```text
inline HTML
    → tag除去してtextだけ

block HTML
    → text fallback
```

とする。

HTML rendererは実装しない。

---

## Part V — LayoutとRender

### 16. Layoutモデル

ASTから直接ANSIを書かない。

```rust
pub struct LayoutDocument {
    pub blocks: Vec<LayoutBlock>,
}
```

### 17. LayoutBlock

```rust
pub enum LayoutBlock {
    Text(TextBlock),
    Code(CodeLayout),
    Table(TableLayout),
    Image(ImageLayout),
    Rule(RuleLayout),
}
```

最終的にはすべて「セル単位の領域」として扱う。

### 18. Text Layout

```rust
pub struct TextBlock {
    pub lines: Vec<LayoutLine>,
}
```

```rust
pub struct LayoutLine {
    pub spans: Vec<LayoutSpan>,
}
```

```rust
pub struct LayoutSpan {
    pub content: String,
    pub style: SemanticStyle,
    pub link: Option<LinkTarget>,
}
```

### 19. SemanticStyle

色をAST内に直接持たない。

```rust
pub enum SemanticStyle {
    Body,
    Muted,

    Heading1,
    Heading2,
    Heading3,
    Heading4,
    Heading5,
    Heading6,

    Strong,
    Emphasis,
    Strike,

    InlineCode,

    Link,

    Quote,

    Code,

    Border,

    AlertNote,
    AlertTip,
    AlertImportant,
    AlertWarning,
    AlertCaution,
}
```

`Border` は §61 の `Theme.border` に対応し、見出しの罫線・code blockの枠・alertの枠で使う。

Theme側で実際の色へ変換。

### 20. Unicode width

Rustの、

```rust
str.len()
```

を絶対に表示幅計算に使わない。

日本語・絵文字を考慮して、

```text
unicode-width
unicode-segmentation
```

を利用する。

文字列wrapはgrapheme cluster単位。

特に、

```text
日本語
🙂
👨‍💻
é
```

をテスト対象とする。

### 21. Layout Context

```rust
pub struct LayoutContext {
    pub terminal_width: u16,
    pub content_width: u16,
}
```

`LayoutContext` は幅の情報だけを持つ。

`Theme` はrender段階の責務（§19）であり、`TerminalCapabilities` はmdsee-terminalの型である。layoutへ載せると §101 の依存方向（layout → terminal 禁止）に反するため、どちらも持たない。

画像配置（S4-9）が必要とする情報は、natural pixel寸法などのmetadataとして個別に渡す。`TerminalCapabilities` 自体は渡さない。

本文幅：

```text
content_width =
    min(
        terminal_width - horizontal_margin,
        config.layout.max_width
    )
```

デフォルト：

```text
max_width = 100
```

### 22. Margin

デフォルト：

```text
left margin = 2
right margin = 2
```

`config.layout.margin`（§64）で変更できる。

Print modeでは中央寄せしない。center layoutはReaderのみ許可する。

### 23. Paragraph Wrapping

word wrappingとCJK wrappingを両立する。

英語：

```text
word boundary
```

日本語：

```text
grapheme boundary
```

を使用。

URLやlong identifierについては、

```text
soft break possible
```

として扱う。

### 24. Heading描画

H1：

```text
MDSEE
━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

H2：

```text
Installation
────────────────────────────
```

H3以下：

```text
### Configuration
```

のように簡潔にする。

TerminalでH1〜H6を全部派手にしない。

### 25. List

Bullet：

```text
• item
  • child
```

Ordered：

```text
1. foo
2. bar
```

Task：

```text
☐ todo
☑ done
```

Unicode不可の場合：

```text
[ ] todo
[x] done
```

### 26. Blockquote

```text
│ quoted text
│ continues here
```

nested quote：

```text
│ │ nested
```

### 27. Alert

GFM：

```markdown
> [!WARNING]
> Dangerous
```

表示：

```text
╭─ WARNING ─────────────────
│ Dangerous
╰───────────────────────────
```

### 28. Code Block

```rust
pub struct CodeBlock {
    pub language: Option<String>,
    pub source: String,
}
```

Render：

```text
╭─ rust ────────────────────────────
│ fn main() {
│     println!("hello");
│ }
╰───────────────────────────────────
```

### 29. Syntax Highlight

`SyntectHighlighter` を作る。

```rust
pub trait SyntaxHighlighter {
    fn highlight(
        &self,
        code: &str,
        language: Option<&str>,
    ) -> Vec<HighlightedLine>;
}
```

Rendererがsyntectへ直接依存しない。

### 30. Table

Tableは難所なので独立moduleにする。

```rust
pub struct TableLayoutEngine;
```

アルゴリズム：

1. 各セルのminimum widthを取得
2. preferred widthを取得
3. columnごとのpreferred width決定
4. 全幅がcontent_widthを超える場合縮小
5. 各cellをwrap
6. row height決定
7. border描画

### 31. Table Width

各column：

```rust
struct ColumnMetrics {
    min: usize,
    preferred: usize,
    assigned: usize,
}
```

縮小時は、

```text
preferred - min
```

の余裕が大きいcolumnから削る。

均等割りにはしない。

### 32. Table Alignment

GFMの、

```markdown
| Left | Center | Right |
| :--- | :----: | ----: |
```

をサポート。

### 33. Hyperlink

OSC 8対応：

```text
ESC ] 8 ; ; URL ST
text
ESC ] 8 ; ; ST
```

TerminalCapabilitiesで対応判定する。

plain fallback：

```text
OpenAI <https://openai.com>
```

---

## Part VI — Terminal capabilities

### 34. TerminalCapabilities

```rust
pub struct TerminalCapabilities {
    pub tty: bool,

    pub columns: u16,
    pub rows: u16,

    pub color_level: ColorLevel,

    pub osc8: bool,

    pub graphics: Vec<GraphicsProtocol>,

    pub cell_pixels: Option<CellPixelSize>,
}
```

### 35. ColorLevel

```rust
pub enum ColorLevel {
    None,
    Ansi16,
    Ansi256,
    TrueColor,
}
```

環境変数：

```text
NO_COLOR
COLORTERM
TERM
TERM_PROGRAM
```

を考慮する。

`NO_COLOR` を優先。

### 36. GraphicsProtocol

```rust
pub enum GraphicsProtocol {
    Kitty,
    Iterm2,
    Sixel,
    Unicode,
}
```

自動選択：

```text
Kitty
 ↓
iTerm2
 ↓
Sixel
 ↓
Unicode
 ↓
None
```

ただしTerminalごとにoverride tableを持つ。

### 37. Terminal detection

環境変数だけに依存しない。

二段階。

```text
1. passive detection
2. optional active query
```

通常起動はpassive。

Reader / `--detect-terminal` 時のみactive query可能。

---

## Part VII — Image pipeline

### 38. ImageSource

```rust
pub enum ImageSource {
    Local(PathBuf),
    Remote(Url),
    Data(Vec<u8>),
}
```

### 39. Image Pipeline

```text
ImageSource
 ↓
ImageLoader
 ↓
DecodedImage
 ↓
ImageSizing
 ↓
RasterImage
 ↓
GraphicsBackend
 ↓
Terminal
```

### 40. ImageBackend

```rust
pub trait ImageBackend {
    fn protocol(&self) -> GraphicsProtocol;

    fn render(
        &mut self,
        image: &RasterImage,
        placement: &ImagePlacement,
    ) -> Result<RenderedImage>;
}
```

### 41. Image sizing

Markdown画像はTerminal幅に合わせて縮小する。

```text
target columns
    = min(
        image natural width,
        content width
      )
```

pixel size取得可能なら、

```text
target_pixel_width
    = columns × cell_pixel_width
```

に変換。

aspect ratioを維持。

### 42. Pixel Geometry

端末の、

```text
cell width px
cell height px
```

が取得できない場合、

画像プロトコル側のcell単位resizeを優先。

Sixelでは推定値fallbackを用意する。

### 43. SVG

```text
SVG
 ↓
usvg parse
 ↓
resvg render
 ↓
RGBA
```

SVG内部から外部URLを取得させない。

### 44. Animated GIF

v0.x：

```text
first frame only
```

将来的にanimation対応可能。

### 45. Kitty Backend

可能であれば、

```text
transmit image
 ↓
receive image id
 ↓
placement
```

を分離。

Readerで同じ画像をスクロールするたびに再転送しない。

### 46. iTerm2 Backend

OSC 1337を使う。

画像データをbase64化して出力。

サイズ指定はcellベースを優先。

### 47. Sixel Backend

独自実装を最初から書かない。

Sixel encoder crateをadapterの裏に隠す。

```rust
pub struct SixelBackend {
    encoder: Box<dyn SixelEncoder>,
}
```

将来的なencoder差し替えを可能にする。

### 48. Unicode Image Fallback

最終fallbackとして画像を小さくrenderし、

```text
▀ ▄ █
```

とTrueColorを使う。

2pixelを1cellで表現。

```text
foreground = upper pixel
background = lower pixel
glyph = ▀
```

これだけでもSixel非対応端末でかなり見られる。

### 49. Remote Image Security

HTTP client：

```text
redirect max = 3
timeout = 5 sec
max download = 20 MB
```

拒否：

```text
localhost
127.0.0.0/8
::1
10.0.0.0/8
172.16.0.0/12
192.168.0.0/16
169.254.0.0/16
```

DNS rebindingも考慮し、名前解決後IPをチェックする。

### 50. Image Cache

キャッシュ：

```text
$XDG_CACHE_HOME/mdsee/
```

macOSではOS標準cache directoryを利用。

key：

```text
SHA256(
 source bytes
 target width
 target height
 protocol
 renderer version
)
```

---

## Part VIII — Reader

### 51. Reader State

```rust
pub struct ReaderState {
    pub document: LayoutDocument,

    pub viewport: Viewport,

    pub scroll_y: usize,
    pub scroll_x: usize,

    pub search: SearchState,
    pub toc: TocState,

    pub selected_link: Option<usize>,
}
```

### 52. Viewport

```rust
pub struct Viewport {
    pub width: u16,
    pub height: u16,
}
```

Terminal resize：

```text
Resize Event
 ↓
layout invalidation
 ↓
relayout
 ↓
preserve logical position
```

単純なrow番号保持ではなく、

```text
Block ID + offset
```

を保持する。

### 53. Reader keybind

```text
q       quit

j       down
k       up

Ctrl-d  half page down
Ctrl-u  half page up

Space   page down
b       page up

g       top
G       bottom

/       search
n       next
N       previous

t       TOC

o       links

h/l     horizontal scroll
```

Vim / lessの中間。

### 54. Search

レンダリング済みtextに対して行う。

```rust
pub struct SearchIndex {
    entries: Vec<SearchEntry>,
}
```

```rust
pub struct SearchEntry {
    block_id: BlockId,
    text: String,
}
```

結果位置からlayout lineへ変換。

### 55. TOC

Headingから生成。

```rust
pub struct TocEntry {
    pub level: u8,
    pub title: String,
    pub block_id: BlockId,
}
```

### 56. Link Navigator

Visible viewport内のlinkを収集。

`o`：

```text
[1] GitHub
[2] Documentation
[3] Image
```

1〜9なら即open。

10以上はselectorを表示。

### 57. External Browser

platform abstraction：

```rust
pub trait UrlOpener {
    fn open(&self, url: &str) -> Result<()>;
}
```

macOS：

```text
open
```

Linux：

```text
xdg-open
```

Windows：

```text
start
```

### 58. Watch mode

```bash
mdsee --watch README.md
```

`notify` で監視。

```text
filesystem event
 ↓
100ms debounce
 ↓
reload
 ↓
parse
 ↓
layout
 ↓
render
```

初期版はfull re-renderでよい。

---

## Part IX — Rich Document

### 59. Mermaid

v0.4では外部 `mmdc` を利用。

```text
mermaid source
 ↓
mmdc
 ↓
SVG
 ↓
mdsee image pipeline
```

存在確認：

```text
PATH
```

見つからない場合：

```text
[mermaid diagram — install mermaid-cli to render]
```

Markdown表示自体を失敗させない。

### 60. Math

初期版はexternal renderer。

```text
Math
 ↓
Typst or KaTeX
 ↓
SVG
 ↓
Image pipeline
```

inline mathは画像化するとline heightが壊れやすいため、

v0.4ではblock mathから始める。

---

## Part X — Theme / Config / Error / 出力規約

### 61. Theme

```rust
pub struct Theme {
    pub body: TextStyle,
    pub muted: TextStyle,

    pub h1: TextStyle,
    pub h2: TextStyle,
    pub h3: TextStyle,

    pub link: TextStyle,

    pub inline_code: TextStyle,

    pub quote: TextStyle,

    pub border: TextStyle,

    pub alerts: AlertTheme,

    pub syntax_theme: String,
}
```

### 62. Built-in Theme

最低限：

```text
auto
dark
light
```

だけでよい。

初期段階から大量themeを入れない。

### 63. Config

パス：

```text
Linux:
~/.config/mdsee/config.toml

macOS:
~/Library/Application Support/mdsee/config.toml
```

ただしXDG_CONFIG_HOMEが設定されていれば優先してもよい。

### 64. config.toml

```toml
theme = "auto"

[layout]
max_width = 100
margin = 2

[reader]
mouse = true

[images]
enabled = true
backend = "auto"
max_height = 40

[network]
remote_images = true
timeout_seconds = 5
max_download_mb = 20
```

### 65. Config priority

```text
CLI
 ↓
environment
 ↓
config file
 ↓
defaults
```

### 66. Error設計

library crateは `thiserror`。

CLI境界だけ `anyhow`。

例：

```rust
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid markdown source")]
    InvalidSource,
}
```

### 67. Error UX

Markdownの一部が失敗してもDocument全体を失敗させない。

例えば画像取得失敗：

```text
[image unavailable: architecture.png]
```

Mermaid失敗：

```text
[failed to render mermaid diagram]
```

exit code 1にする必要はない。

### 68. Exit Code

```text
0 success
1 runtime error
2 CLI argument error
```

remote image取得失敗などnon-critical errorは0。

### 69. Logging

通常はログを一切出さない。

```bash
mdsee --debug
```

の場合のみstderr。

内部は `tracing` を利用してよい。

### 70. stdout / stderr

厳密に分離。

```text
stdout
    document output

stderr
    diagnostics
```

これにより、

```bash
mdsee README.md > rendered.txt
```

を壊さない。

### 71. Pipe Behavior

stdoutがTTYではない場合：

```text
ANSI OFF
OSC8 OFF
images OFF
reader OFF
```

Markdownをplain textへrender。

### 72. NO_COLOR

`NO_COLOR` が存在すれば、

```text
color = false
```

CLIの、

```bash
--force-color
```

だけがoverride可能。

---

## Part XI — アーキテクチャと性能

### 73. Architecture Pipeline

最終的な処理：

```text
CLI Args
    │
    ▼
Config Loader
    │
    ▼
Terminal Detector
    │
    ▼
Input Loader
    │
    ▼
Markdown Parser
    │
    ▼
Internal Document AST
    │
    ▼
Asset Resolver
    │
    ▼
Layout Engine
    │
    ▼
Layout Document
    │
    ├──────────────┐
    ▼              ▼
Print Renderer   Reader
    │              │
    ├─────┬────────┤
    ▼     ▼        ▼
ANSI   Images    TUI
    │     │        │
    └─────┴────────┘
          │
          ▼
       Terminal
```

### 74. 非同期処理

v0.1は同期でよい。

Remote image導入時にTokio runtimeを全面導入するとCLIサイズ・複雑性が増える。

方針：

```text
core pipeline
    sync

optional network
    blocking reqwest
```

から開始する。

大量画像など必要になったらasync化を検討。

### 75. ID設計

各Blockに、

```rust
pub struct BlockId(u64);
```

を付ける。

Reader、search、TOC、source mapに共通利用する。

Parse時に連番発行。

### 76. キャッシュ戦略

三種類を分離。

```text
Image Cache
Rendered Asset Cache
Document Parse Cache
```

ただしv0.xはImage Cacheのみ。

Markdown解析は十分高速。

### 77. Performance目標

画像なし：

```text
10 KB    < 15 ms
100 KB   < 50 ms
1 MB     < 250 ms
```

Reader first paint：

```text
< 100 ms
```

を目標。

### 78. Memory目標

通常README：

```text
< 30 MB RSS
```

画像なし。

巨大Markdownでも入力サイズに比例して極端に増えないこと。

---

## Part XII — テスト / CI / リリース

### 79. テスト戦略

4層。

```text
Unit
Snapshot
Integration
Terminal Compatibility
```

### 80. Unit Test

対象：

```text
parser
wrapping
unicode width
table width
link resolution
image sizing
config merge
terminal detection
```

### 81. Snapshot Test

Markdown fixture：

```text
tests/fixtures/
├── headings.md
├── tables.md
├── japanese.md
├── emoji.md
├── code.md
├── alerts.md
└── complete.md
```

ANSIをnormalizedしてsnapshot比較。

### 82. Terminal Snapshot

Escape sequenceそのものもテストする。

例：

```text
OSC 8
Kitty command
iTerm2 command
Sixel command
```

Terminal emulatorそのものに依存せずProtocol outputを検証。

### 83. Integration Test

```bash
mdsee tests/fixtures/basic.md
```

を実行して、

```text
exit status
stdout
stderr
```

を確認。

### 84. 日本語テスト

mdseeでは重要。

以下は必須。

```text
日本語文章の折り返し
全角記号
半角英数字混在
絵文字
結合文字
コード内日本語
日本語表
```

### 85. CI

GitHub Actions：

```text
Ubuntu
macOS
Windows
```

で、

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release
```

### 86. Release Build

target：

```text
aarch64-apple-darwin
x86_64-apple-darwin

x86_64-unknown-linux-gnu
aarch64-unknown-linux-gnu

x86_64-pc-windows-msvc
```

### 87. Release Artifact

名前を統一。

```text
mdsee-v0.1.0-aarch64-apple-darwin.tar.gz
mdsee-v0.1.0-x86_64-apple-darwin.tar.gz
mdsee-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
```

中身：

```text
mdsee
README.md
LICENSE
```

### 88. Homebrew

別repository：

```text
homebrew-tap
```

構造：

```text
homebrew-tap/
└── Formula/
    └── mdsee.rb
```

インストール：

```bash
brew install OWNER/tap/mdsee
```

### 89. Homebrew Formula

release時にGitHub Actionsから、

```text
version
URL
SHA256
```

を自動更新する。

手動更新しない。

### 90. Cargo Install

同時に、

```bash
cargo install mdsee
```

をサポート。

crates.io package名も可能なら `mdsee` を確保する。

### 91. Binary Size

機能追加で巨大化しやすい。

release profile：

```toml
[profile.release]
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

### 92. Feature Flags

画像関係をfeature分離。

```toml
[features]
default = ["syntax"]

syntax = []
images = []
sixel = ["images"]
svg = ["images"]
reader = []
network = []
mermaid = ["images"]
math = ["images"]
```

ただしHomebrew buildでは全部入りbinaryを配布。

---

## Part XIII — 公開Library API

### 93. Public Library API

最初から、

```rust
mdsee_core
mdsee_layout
mdsee_render
```

は再利用可能にしておく。

例えば将来的に、

```rust
let document = mdsee::parse(markdown)?;
let rendered = mdsee::render(document, options)?;
```

と使える。

### 94. CLIとLibraryの境界

絶対に、

```rust
std::process::exit()
```

をlibrary crate内で使わない。

CLIのみでexitする。

### 95. 将来的なRenderer Backend

Layoutを抽象化しておけば、

```text
ANSI Renderer
HTML Renderer
SVG Renderer
Plain Renderer
```

を作れる。

ただし初期実装ではANSI / Plainのみ。

---

## Part XIV — ロードマップと設計原則

### 96. 実装順序

#### Sprint 1

```text
Workspace
CLI
stdin/file
Comrak
Internal AST
Paragraph
Heading
Inline Style
ANSI
```

この時点で、

```bash
mdsee README.md
```

が動く。

#### Sprint 2

```text
lists
quote
rules
links
OSC8
theme
TTY
plain mode
```

#### Sprint 3

```text
syntax highlight
code blocks
tables
GFM alerts
Unicode edge cases
```

ここでv0.1。

#### Sprint 4

```text
TerminalCapabilities
ImageSource
Image sizing
Kitty
iTerm2
```

#### Sprint 5

```text
Sixel
SVG
Unicode image fallback
cache
```

ここでv0.2。

#### Sprint 6

```text
Ratatui Reader
scroll
resize
search
TOC
links
```

v0.3。

#### Sprint 7

```text
remote image
Mermaid
Math
Watch
```

v0.4。

ロードマップの詳細なタスク分解は [implementation-plan.md](./implementation-plan.md) を参照。

### 97. v0.1 Acceptance Criteria

次が全部通ったらv0.1。

```bash
mdsee README.md

cat README.md | mdsee

mdsee japanese.md

mdsee table.md

mdsee code.md

mdsee README.md > out.txt
```

最後の`out.txt`にANSI escapeが入らないこと。

### 98. v0.2 Acceptance Criteria

最低限以下で画像を確認。

```text
Ghostty
Kitty
iTerm2
WezTerm
foot
```

全Terminalで同じProtocolを使おうとしない。

TerminalCapabilitiesによるbackend選択が正しいことを確認。

### 99. 初期段階でやらないこと

以下は意図的に後回し。

```text
Markdown Editor
Markdown editing
Browser preview
HTML完全互換
CSS
Plugin system
GitHub API
LLM integration
PDF
Presentation mode
Full Mermaid implementation
Custom Sixel codec
```

プロジェクトを肥大化させない。

### 100. 最初に作るべき内部API

実装開始時はこの5つを先に固定する。

```rust
pub fn load_source(
    input: InputSource
) -> Result<SourceDocument>;

pub fn parse(
    source: &SourceDocument
) -> Result<Document>;

pub fn layout(
    document: &Document,
    options: &LayoutOptions
) -> Result<LayoutDocument>;

pub fn detect_terminal()
    -> TerminalCapabilities;

pub fn render(
    document: &LayoutDocument,
    target: &mut dyn Write,
    options: &RenderOptions,
) -> Result<()>;
```

これがmdseeの基本pipeline。

### 101. 最重要の依存方向

依存方向を厳守する。

```text
core
 ↑
layout
 ↑
render

terminal ← render
terminal ← image

layout ← image metadata

reader
 ├─ layout
 ├─ render
 ├─ terminal
 └─ image

cli
 └─ everything
```

逆依存を作らない。

特に、

```text
core → terminal
```

は禁止。

### 102. 最終的なプロダクト形

ユーザーから見える機能は複雑に見せない。

基本は永久に、

```bash
mdsee README.md
```

だけでよい。

必要な場合のみ、

```bash
mdsee -r README.md
mdsee -w README.md
mdsee --graphics sixel README.md
```

を使う。

内部は高度でも、CLIは極力単純に維持する。

### 103. プロジェクトの設計原則

実装時に以下をDesign PrinciplesとしてREADMEにも残す。

1. **CLI first**
   アプリではなくUnix commandとして振る舞う。

2. **Text stays text**
   Markdown本文を画像化しない。

3. **Progressive enhancement**
   ANSI → TrueColor → Graphics Protocolと端末性能に応じて強化する。

4. **Graceful degradation**
   KittyやSixelがなくても壊れない。

5. **Pipe safe**
   stdout redirectを壊さない。

6. **Fast startup**
   Markdownを見るために待たせない。

7. **Library first architecture**
   CLI固有処理をcore rendererへ混ぜない。

8. **Terminal native**
   ブラウザをTerminalに再実装するのではなく、TerminalらしいDocument Viewerを作る。

### 104. 実装開始時の完成イメージ

最初のゴールはこれ。

```text
$ mdsee README.md

  mdsee
  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Beautiful Markdown in your terminal.

  Installation
  ─────────────────────────────────────────────

  ╭─ shell ────────────────────────────────────
  │ brew install owner/tap/mdsee
  ╰────────────────────────────────────────────

  Features

  • GFM Markdown
  • Syntax highlighting
  • Terminal hyperlinks
  • Images with Kitty / Sixel / iTerm2
  • Tables
  • Reader mode

  Documentation
  https://github.com/owner/mdsee
```

そしてv0.2ではMarkdown中に、

```markdown
![Screenshot](assets/screenshot.png)
```

が存在すれば、その位置へTerminal Graphics Protocolで実画像が挿入される。

これを `mdsee` の最初の明確な完成形とする。
