> mdsee — Beautiful Markdown in your terminal.

# mdsee

Markdown をターミナルで高品質に表示する CLI ビューア。

```bash
mdsee README.md
cat README.md | mdsee
```

## インストール

### Cargo

Rust/Cargo がインストールされている場合:

```bash
cargo install mdsee
```

### Homebrew

Homebrew の作者タップを追加してインストールします:

```bash
brew tap kotsutsumi/tap
brew install kotsutsumi/tap/mdsee
```

設計と実装計画は [docs/design.md](./docs/design.md) と [docs/implementation-plan.md](./docs/implementation-plan.md) を参照。

## License

MIT
