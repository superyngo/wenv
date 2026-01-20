# Pending Entry 狀態機

本文件詳細說明 wenv Parser 中的 pending entry 狀態機，用於處理 Comment 和 Code 類型 entry 的合併邏輯。

---

## 1. 概述

### 為什麼需要狀態機？

在 shell 配置檔中，Comment 和 Code 經常需要合併處理：

```bash
# Section header     ← Comment
                     ← 空行（應該被吸收）
if [ -f file ]; then ← 控制結構開始
    echo "hi"
fi                   ← 控制結構結束
                     ← 尾部空行（應該被吸收）
```

如果逐行輸出，會產生過多碎片化的 entry。狀態機允許**延遲輸出**，讓後續行有機會與前一行合併。

### 核心概念

```
pending_entry: Option<Entry>
```

- **None**：無待處理 entry
- **Some(entry)**：有一個 entry 等待決定是否合併或輸出

每遇到新行時，根據 `pending_entry` 的類型和新行的類型，決定：
1. **合併**：將新行合併到 pending entry
2. **輸出**：將 pending entry 加入結果，新行變成新的 pending
3. **直接輸出**：pending 和新行都直接加入結果（structured entry）

---

## 2. 狀態轉移表

### 2.1 空行（Blank）遇到 pending

| pending 類型 | 動作 | 結果 |
|-------------|------|------|
| `None` | 空行變成 pending | `pending = Some(blank)` |
| `Comment` | 吸收空行 | `pending.merge_trailing(blank)` |
| `Code (blank)` | 合併空行 | `pending.merge_trailing(blank)` |
| `Code (non-blank)` | 吸收尾部空行 | `pending.merge_trailing(blank)` |
| 其他 | 輸出 pending，空行變成新 pending | `flush + pending = Some(blank)` |

### 2.2 Comment 遇到 pending

| pending 類型 | 動作 | 結果 |
|-------------|------|------|
| `None` | Comment 變成 pending | `pending = Some(comment)` |
| `Comment` | 合併 Comment | `pending.merge_trailing(comment)` |
| 其他 | 輸出 pending，Comment 變成新 pending | `flush + pending = Some(comment)` |

### 2.3 非空白 Code 遇到 pending

| pending 類型 | 動作 | 結果 |
|-------------|------|------|
| `None` | Code 變成 pending | `pending = Some(code)` |
| `Comment` | **類型升級**，合併為 Code | `pending.merge_trailing(code)` → pending 變成 Code |
| `Code (blank)` | 輸出 pending，Code 變成新 pending | `flush + pending = Some(code)` |
| `Code (non-blank)` | 輸出 pending，Code 變成新 pending | `flush + pending = Some(code)` |
| 其他 | 輸出 pending，Code 變成新 pending | `flush + pending = Some(code)` |

### 2.4 控制結構開始遇到 pending

| pending 類型 | 動作 | 結果 |
|-------------|------|------|
| `None` | 開始新 CodeBlockBuilder | `block = new(line_number)` |
| `Comment` | **合併到控制塊** | pending 內容成為 block 開頭 |
| `Code` | **合併到控制塊** | pending 內容成為 block 開頭 |
| 其他 | 輸出 pending，開始新 block | `flush + block = new(line_number)` |

### 2.5 控制結構結束

| 動作 | 結果 |
|------|------|
| 控制塊完成 | `pending = Some(block.build())` |

控制塊結束後**不直接輸出**，而是變成 pending，這樣可以吸收後續的空行。

### 2.6 Structured Entry（Alias/EnvVar/Source/Function）遇到 pending

| pending 類型 | 動作 | 結果 |
|-------------|------|------|
| 任何 | 輸出 pending，直接輸出 structured entry | `flush pending + add entry` |

Structured entry 永遠不會變成 pending，也不會與 pending 合併。

---

## 3. 狀態轉移圖

```
                                    ┌─────────────────────────────────────┐
                                    │                                     │
                                    ▼                                     │
┌──────────┐    blank/comment    ┌──────────────┐                        │
│   None   │ ─────────────────→  │   Pending    │                        │
└──────────┘                     │  (Comment/   │                        │
     ▲                           │   Code)      │                        │
     │                           └──────────────┘                        │
     │                                 │                                  │
     │   structured entry              │ structured entry                 │
     │   (直接輸出)                    │ (flush + 直接輸出)                │
     │                                 ▼                                  │
     │                           ┌──────────────┐                        │
     └───────────────────────────│    Flush     │────────────────────────┘
                                 │   + Output   │
                                 └──────────────┘
```

### Pending 內部轉移

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Pending Entry 內部狀態                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌──────────────┐                                                      │
│   │   Comment    │                                                      │
│   └──────────────┘                                                      │
│         │                                                                │
│         │ + comment  → merge (stay Comment)                             │
│         │ + blank    → merge (stay Comment, absorb blank)               │
│         │ + code     → merge + upgrade to Code ──────────┐              │
│         │ + control  → merge into control block          │              │
│         │                                                 │              │
│         ▼                                                 ▼              │
│   ┌──────────────┐                              ┌──────────────┐        │
│   │  Flush +     │                              │     Code     │        │
│   │  New Pending │                              │ (upgraded)   │        │
│   └──────────────┘                              └──────────────┘        │
│                                                        │                 │
│                                                        │ + blank → merge │
│                                                        │ + code  → flush │
│                                                        ▼                 │
│                                                  ┌──────────────┐       │
│                                                  │  Flush +     │       │
│                                                  │  New Pending │       │
│                                                  └──────────────┘       │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 4. 合併規則詳解

### 4.1 Comment + Comment → Comment

```bash
# Line 1        ← pending = Comment(#L1)
# Line 2        ← pending.merge_trailing() → Comment(#L1-L2)
# Line 3        ← pending.merge_trailing() → Comment(#L1-L3)
```

**結果**：單一 Comment entry，包含所有相鄰註解行

### 4.2 Comment + Blank → Comment（吸收空行）

```bash
# Header        ← pending = Comment(#L1)
                ← pending.merge_trailing() → Comment(#L1-L2)
                ← pending.merge_trailing() → Comment(#L1-L3)
alias ll='ls'   ← flush Comment(#L1-L3)，輸出 Alias
```

**結果**：Comment 吸收了後續的空行

### 4.3 Comment + Code → Code（類型升級）

```bash
# Note          ← pending = Comment(#L1)
echo "hello"    ← pending.merge_trailing() → Code(L1-L2)，類型升級
                ← pending.merge_trailing() → Code(L1-L3)，吸收空行
alias ll='ls'   ← flush Code(L1-L3)，輸出 Alias
```

**類型升級規則**：
```rust
if self.entry_type == EntryType::Comment
    && other.entry_type == EntryType::Code
    && !other.value.is_empty()  // 非空白 Code
{
    self.entry_type = EntryType::Code;  // 升級為 Code
}
```

### 4.4 Comment + Control Structure → Code

```bash
# Section       ← pending = Comment(#L1)
                ← pending.merge_trailing() → Comment(#L1-L2)
if true; then   ← 控制結構開始，pending 合併到 block
    echo hi
fi              ← 控制結構結束 → pending = Code(L1-L5)
                ← pending.merge_trailing() → Code(L1-L6)
alias ll='ls'   ← flush Code(L1-L6)，輸出 Alias
```

### 4.5 Blank + Blank → Code（合併空行）

```bash
                ← pending = Code(L1, blank)
                ← pending.merge_trailing() → Code(L1-L2, blank)
                ← pending.merge_trailing() → Code(L1-L3, blank)
alias ll='ls'   ← flush Code(L1-L3)，輸出 Alias
```

### 4.6 Blank + Code → 不合併（分開）

```bash
                ← pending = Code(L1, blank)
echo "hello"    ← flush Code(L1)，pending = Code(L2)
```

**重要**：空行開頭的 pending 遇到非空白 Code 時，不合併，而是分開輸出。

### 4.7 Code + Blank → Code（吸收尾部空行）

```bash
echo "hello"    ← pending = Code(L1)
                ← pending.merge_trailing() → Code(L1-L2)
                ← pending.merge_trailing() → Code(L1-L3)
alias ll='ls'   ← flush Code(L1-L3)，輸出 Alias
```

### 4.8 Code + Code → 不合併（分開）

```bash
echo "line1"    ← pending = Code(L1)
echo "line2"    ← flush Code(L1)，pending = Code(L2)
```

**原因**：兩個獨立的程式碼行應該保持分開，便於個別編輯。

---

## 5. 控制結構特殊處理

### 5.1 控制結構開始時合併 pending

```rust
if current_code_block.is_none() && prev_depth == 0 && control_depth > 0 {
    if let Some(pending) = pending_entry.take() {
        if matches!(pending.entry_type, EntryType::Comment | EntryType::Code) {
            // 將 pending 內容加入 block 開頭
            let start_line = pending.line_number.unwrap_or(line_number);
            let mut block = CodeBlockBuilder::new(start_line);

            // 逐行加入 pending 的 raw_line
            if let Some(ref raw) = pending.raw_line {
                for l in raw.split('\n') {
                    block.add_line(l);
                }
            }
            block.add_line(line);  // 加入控制結構開始行
            current_code_block = Some(block);
        }
    }
}
```

### 5.2 控制結構結束時變成 pending

```rust
if prev_depth > 0 && control_depth == 0 {
    if let Some(block) = current_code_block.take() {
        pending_entry = Some(block.build());  // 不直接輸出，變成 pending
    }
}
```

**原因**：控制結構結束後可能有尾部空行需要吸收。

---

## 6. merge_trailing() 實現

```rust
pub fn merge_trailing(&mut self, other: Entry) {
    // 1. 更新 end_line
    self.end_line = other.end_line.or(other.line_number);

    // 2. 合併 raw_line
    if let Some(ref mut raw) = self.raw_line {
        if let Some(other_raw) = other.raw_line {
            raw.push('\n');
            raw.push_str(&other_raw);
        }
    } else {
        self.raw_line = other.raw_line;
    }

    // 3. 類型升級：Comment + non-blank Code → Code
    if self.entry_type == EntryType::Comment
        && other.entry_type == EntryType::Code
        && !other.value.is_empty()
    {
        self.entry_type = EntryType::Code;
        // 保留原始 Comment 的 value（用於 TUI 顯示）
    }

    // 4. 更新 name 反映新行範圍
    if let (Some(start), Some(end)) = (self.line_number, self.end_line) {
        let prefix = if self.entry_type == EntryType::Comment { "#L" } else { "L" };
        if start == end {
            self.name = format!("{}{}", prefix, start);
        } else {
            self.name = format!("{}{}-L{}", prefix, start, end);
        }
    }
}
```

---

## 7. 完整範例

### 輸入

```bash
# Git aliases
# for daily use

alias gs='git status'
alias gd='git diff'

# Conditional setup

if [ -f ~/.local_config ]; then
    source ~/.local_config
fi

echo "done"
```

### 處理過程

| 行號 | 內容 | pending 狀態 | 動作 |
|------|------|-------------|------|
| 1 | `# Git aliases` | None → Comment(#L1) | 新建 pending |
| 2 | `# for daily use` | Comment(#L1) → Comment(#L1-L2) | 合併 |
| 3 | (blank) | Comment(#L1-L2) → Comment(#L1-L3) | 吸收空行 |
| 4 | `alias gs='git status'` | flush Comment(#L1-L3) | 輸出 Comment，輸出 Alias |
| 5 | `alias gd='git diff'` | None | 直接輸出 Alias |
| 6 | (blank) | None → Code(L6, blank) | 新建 pending |
| 7 | `# Conditional setup` | flush Code(L6), Comment(#L7) | 輸出空行，新建 Comment |
| 8 | (blank) | Comment(#L7) → Comment(#L7-L8) | 吸收空行 |
| 9 | `if [ -f ... ]; then` | Comment(#L7-L8) 合併到 block | 控制結構開始 |
| 10 | `    source ...` | (in block) | 繼續累積 |
| 11 | `fi` | (in block) → pending Code(L7-L11) | 控制結構結束 |
| 12 | (blank) | Code(L7-L11) → Code(L7-L12) | 吸收尾部空行 |
| 13 | `echo "done"` | flush Code(L7-L12), Code(L13) | 輸出，新建 pending |
| EOF | - | flush Code(L13) | 輸出最後 pending |

### 輸出 Entry 列表

1. `Comment(#L1-L3)` - 包含 "# Git aliases", "# for daily use", 空行
2. `Alias(gs)` - git status
3. `Alias(gd)` - git diff
4. `Code(L6)` - 空行
5. `Code(L7-L12)` - 包含 "# Conditional setup", 空行, if...fi, 尾部空行
6. `Code(L13)` - echo "done"

---

## 8. 設計考量

### 為什麼 Comment 可以被升級為 Code？

當 Comment 後面跟著非結構化的程式碼時，通常它們是一個邏輯單元：

```bash
# 下面這行設定環境
export SPECIAL_VAR=1
```

如果 `export SPECIAL_VAR=1` 無法被解析為 EnvVar（例如語法特殊），它會被當作 Code。
此時 Comment + Code 應該合併，因為它們語義上相關。

### 為什麼控制結構結束後要變成 pending？

```bash
if true; then
    echo hi
fi

alias ll='ls'
```

如果 `fi` 後直接輸出，尾部空行會變成獨立的 Code entry。
透過 pending 機制，尾部空行可以被吸收到控制結構中。

### 為什麼 Blank + Code 不合併？

```bash

echo "hello"
```

空行開頭通常表示「新區塊」的開始，不應與後續程式碼合併。
這樣使用者可以獨立編輯/刪除空行。

---

## 9. 程式碼位置

| 功能 | 檔案位置 |
|------|----------|
| pending 狀態機主邏輯 | `src/parser/bash/mod.rs:239-443` |
| merge_trailing() | `src/model/entry.rs:90-136` |
| CodeBlockBuilder | `src/parser/builders/code_block.rs` |
| 控制結構偵測 | `src/parser/bash/control.rs` |
