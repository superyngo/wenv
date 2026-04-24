# feature/tree-sitter 開發意圖整理

## 意圖 (Intent)

**以 tree-sitter CST（具體語法樹）取代當前基於正則表達式的 Bash 解析器**，提升解析的準確性和健壯性。

## 目標 (Goals)

1. **結構化解析**：利用 `tree-sitter-bash` 構建真正的語法樹，而非依賴脆弱的正則匹配
2. **精確分類**：將 CST 節點精確映射為 `EntryType`（Alias / EnvVar / Function / Source / Comment / Code）
3. **復用合併邏輯**：沿用手動解析器（regex parser）的 `PendingBlock` 狀態機來處理註釋吸收、空行合併等邊界語義
4. **漸進式引入**：作為 optional cargo feature (`tree-sitter`) 存在，不影響默認編譯

## 手段 (Approach)

| 層次 | 實現方式 |
|------|---------|
| **依賴** | `tree-sitter = "0.26"` + `tree-sitter-bash = "0.25"`，均 `optional = true` |
| **Feature gate** | `#[cfg(feature = "tree-sitter")]` 隔離，`Cargo.toml` 中 `tree-sitter = ["dep:tree-sitter", "dep:tree-sitter-bash"]` |
| **架構** | `src/parser/ts_bash/` — `mod.rs`（主解析器）+ `classify.rs`（節點分類器） |
| **分類** | `classify_node_with_source()` 遍歷頂層 CST 節點，按 `node.kind()` 分派：`comment` → Comment、`function_definition` → Function、`declaration_command` → EnvVar、`command` → 按指令名區分 Alias/Source/Code |
| **合併** | 解析後的 entry 仍走 `PendingBlock` → `handle_blank` / `handle_comment` / `merge_pending_with_structured` 等既有流程，保證與 regex parser 一致的分組行為 |
| **集成** | `parser/mod.rs` 條件導出 `TsBashParser`，實現 `Parser` trait，可無縫替換 `BashParser` |

## 涉及檔案

- `Cargo.toml` — 新增 optional dependencies + feature flag
- `src/parser/ts_bash/mod.rs` — 主解析器實作（~310 行，含測試）
- `src/parser/ts_bash/classify.rs` — CST 節點分類器（~231 行，含測試）
- `src/parser/mod.rs` — 條件編譯導出 `TsBashParser`
- `Cargo.lock` — 鎖定 tree-sitter 相關依賴版本

## 狀態

所有程式碼停留在 working tree，尚未產生 commit。分支 `feature/tree-sitter` 與 `main` 無差異。
