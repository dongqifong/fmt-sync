# fmt-sync

用同一組指令管理 Python、Go、Rust、TypeScript 專案的格式化與 lint。支援 Windows、Linux。

## 安裝

先安裝 [Rust](https://rust-lang.org/tools/install/)，再於本專案目錄執行：

```sh
cargo install --path . --locked
```

依專案語言安裝需要的工具：

| 語言 | 安裝方式 | 安裝位置 |
| --- | --- | --- |
| Python | `uv add --dev ruff`（專案內）<br>或 `uv tool install ruff`（共用） | 專案 `.venv` 或使用者工具環境 |
| Go | [安裝 Go](https://go.dev/doc/install)，將 `go`、`gofmt` 加入 PATH | 共用 Go 工具鏈，內含 gofmt、go vet |
| Rust | 在專案目錄執行 `rustup component add rustfmt clippy` | 專案目前使用的 Rust 工具鏈；同工具鏈的專案共用 |
| TypeScript | `npm install --save-dev --save-exact @biomejs/biome`（需 Node.js／npm） | 專案 `node_modules` |

缺少工具時會顯示安裝指引，不會自動安裝。
Ruff、Biome 優先使用專案內的工具，再搜尋 PATH；Rust 的工具鏈由 Cargo／rustup 決定。

## 使用

進入要管理的專案目錄：

```sh
fmt-sync init --lang python,typescript
fmt-sync format
fmt-sync check
```

- `init`：建立設定，已有的原生設定會保留。
- `format`：依設定格式化並寫入檔案。
- `check`：檢查格式與 lint，不修改原始碼。

只有 `init` 指定語言；`format`、`check` 都讀取目前目錄的 `fmt-sync.toml`。

### 專案設定

```toml
schema-version = 1
languages = ["python", "typescript"]
ignore = ["legacy/", "experimental/"]
```

每個專案各放自己的 TOML，不向上繼承。`ignore` 是相對於 TOML 的子專案目錄，整個目錄一起跳過，不接受單一檔案。

Go／Rust 請在含有 `go.mod`／`Cargo.toml` 的專案目錄執行。

### 使用自己的 preset

把原生設定檔放入範本目錄，再指定它：

```sh
fmt-sync init --lang python,typescript --preset ../my-preset
```

例如範本目錄可放 `ruff.toml`、`rustfmt.toml`、`biome.json`。未指定 preset，或缺少某語言設定時，使用以下預設。初始化後可直接修改專案內的原生設定檔。

## 預設規則

### Python — Ruff

- **設定檔：** `ruff.toml`
- **格式：** 4 空格縮排、行寬 88，先整理 import 再排版。
- **檢查：** `E4`、`E7`、`E9`、`F`、`I`（基本錯誤、未使用變數／import、import 排序）。
- **既有專案：** 在 `[lint]` 加上 `extend-select = ["I"]` 可啟用排序；`check` 不改檔。
- **快取：** 停用 Ruff 快取，不產生 `.ruff_cache`。

### Go — gofmt + go vet

- **設定檔：** 不需要。
- **格式：** gofmt 標準排版，使用 Tab 縮排。
- **檢查：** go vet 預設檢查。
- **快取：** 預設使用 Go 的系統共用快取。

### Rust — rustfmt + Clippy

- **設定檔：** `rustfmt.toml`
- **格式：** 4 空格縮排、行寬 100。
- **產物：** 保留 Cargo 的建置產物（預設 `target/`），供後續檢查重用；支援 Cargo 自訂目錄。
- **檢查：** Clippy 預設檢查，警告視為不通過。

### TypeScript — Biome

- **設定檔：** `biome.json`
- **格式：** Tab 縮排、行寬 80、雙引號。
- **檢查：** Biome recommended lint 規則。
- **快取：** 一般執行不產生專案快取。
