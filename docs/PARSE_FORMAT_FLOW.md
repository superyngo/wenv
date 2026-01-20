# Parse 與 Format 完整流程

本文件詳細說明 wenv 的 Parser 輸入處理流程和 Formatter 輸出處理流程。

---

## 1. Parse 輸入流程

### 1.1 流程概覽

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Parser 完整流程                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  輸入: content: &str                                                     │
│          │                                                               │
│          ▼                                                               │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                      初始化狀態變數                               │    │
│  │  • in_function, brace_count, current_func                       │    │
│  │  • control_depth, current_code_block                            │    │
│  │  • current_alias, current_env                                   │    │
│  │  • pending_entry, pending_comment                               │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│          │                                                               │
│          ▼                                                               │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │              for (line_num, line) in content.lines()             │    │
│  │                           │                                      │    │
│  │           ┌───────────────┴───────────────┐                     │    │
│  │           ▼                               ▼                     │    │
│  │   ┌──────────────┐               ┌──────────────┐               │    │
│  │   │ 多行狀態處理  │               │  單行判斷    │               │    │
│  │   │ (最高優先級)  │               │              │               │    │
│  │   └──────────────┘               └──────────────┘               │    │
│  │           │                               │                      │    │
│  │           └───────────────┬───────────────┘                     │    │
│  │                           ▼                                      │    │
│  │                    處理結果：                                     │    │
│  │                    • 直接輸出 entry                              │    │
│  │                    • 變成 pending                                │    │
│  │                    • 合併到現有狀態                              │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│          │                                                               │
│          ▼                                                               │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                       清理殘留狀態                                │    │
│  │  • flush pending_entry                                          │    │
│  │  • 警告未閉合的 function/alias/env                              │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│          │                                                               │
│          ▼                                                               │
│  輸出: ParseResult { entries: Vec<Entry>, warnings: Vec<Warning> }      │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 1.2 狀態變數說明

| 變數 | 類型 | 用途 |
|------|------|------|
| `in_function` | `bool` | 是否在函數內部 |
| `brace_count` | `usize` | 函數大括號計數 |
| `current_func` | `Option<FunctionBuilder>` | 正在累積的函數 |
| `control_depth` | `usize` | 控制結構嵌套深度 |
| `current_code_block` | `Option<CodeBlockBuilder>` | 正在累積的控制塊 |
| `current_alias` | `Option<QuotedValueBuilder>` | 正在累積的多行 alias |
| `current_env` | `Option<QuotedValueBuilder>` | 正在累積的多行 export |
| `pending_entry` | `Option<Entry>` | **待決定的 Comment/Code** |
| `pending_comment` | `Option<String>` | 待關聯到下一個 entry 的註解文字 |

### 1.3 主迴圈處理順序（優先級）

```
for each line:
    │
    ├─[1] 多行函數處理 (in_function == true)
    │     └─ continue
    │
    ├─[2] 多行 Alias 處理 (current_alias.is_some())
    │     └─ continue
    │
    ├─[3] 多行 EnvVar 處理 (current_env.is_some())
    │     └─ continue
    │
    ├─[4] 控制結構處理 (control_depth > 0)
    │     └─ continue
    │
    ├─[5] 空行處理 (trimmed.is_empty())
    │     └─ pending 狀態機介入
    │     └─ continue
    │
    ├─[6] Comment 處理 (starts_with '#')
    │     └─ pending 狀態機介入
    │     └─ continue
    │
    ├─[7] 嘗試解析 Alias
    │     └─ 成功: flush pending, 輸出 entry, continue
    │     └─ 多行開始: flush pending, 啟動 builder, continue
    │
    ├─[8] 嘗試解析 EnvVar
    │     └─ 成功: flush pending, 輸出 entry, continue
    │     └─ 多行開始: flush pending, 啟動 builder, continue
    │
    ├─[9] 嘗試解析 Source
    │     └─ 成功: flush pending, 輸出 entry, continue
    │
    ├─[10] 嘗試解析 Function
    │      └─ 成功: flush pending, 啟動 builder, continue
    │
    └─[11] Fallback: 視為 Code
          └─ pending 狀態機介入
```

### 1.4 各步驟詳解

#### [1] 多行函數處理

```rust
if in_function {
    // 計算大括號
    let (open, close) = count_braces_outside_quotes(trimmed);
    brace_count += open;
    brace_count -= close;

    // 累積行
    current_func.add_line(line);

    // 函數結束？
    if brace_count == 0 {
        in_function = false;
        let entry = current_func.take().build(EntryType::Function);
        result.add_entry(entry);
    }
    continue;
}
```

**狀態機介入**：無（函數內不使用 pending）

#### [2] 多行 Alias 處理

```rust
if let Some(ref mut builder) = current_alias {
    builder.add_line(line);

    if builder.is_complete() {  // 單引號配對完成
        let entry = current_alias.take().build(EntryType::Alias);
        result.add_entry(entry);
    }
    continue;
}
```

**狀態機介入**：無

#### [3] 多行 EnvVar 處理

```rust
if let Some(ref mut builder) = current_env {
    builder.add_line(line);

    if builder.is_complete() {
        let entry = current_env.take().build(EntryType::EnvVar);
        result.add_entry(entry);
    }
    continue;
}
```

**狀態機介入**：無

#### [4] 控制結構處理

```rust
// 更新深度
let prev_depth = control_depth;
control_depth -= count_control_end(trimmed);  // fi, done, esac
control_depth += count_control_start(trimmed); // if, while, for, case

if control_depth > 0 || (prev_depth > 0 && control_depth == 0) {
    // 控制結構開始
    if current_code_block.is_none() && prev_depth == 0 && control_depth > 0 {
        // ★ 狀態機介入：合併 pending Comment/Code
        if let Some(pending) = pending_entry.take() {
            if matches!(pending.entry_type, Comment | Code) {
                // pending 內容成為 block 開頭
                block = CodeBlockBuilder::new(pending.line_number);
                for l in pending.raw_line.split('\n') {
                    block.add_line(l);
                }
            } else {
                result.add_entry(pending);  // 非 Comment/Code 直接輸出
                block = CodeBlockBuilder::new(line_number);
            }
        }
        block.add_line(line);
    } else {
        // 控制結構內部：繼續累積
        current_code_block.add_line(line);
    }

    // 控制結構結束
    if prev_depth > 0 && control_depth == 0 {
        // ★ 狀態機介入：不直接輸出，變成 pending
        pending_entry = Some(current_code_block.take().build());
    }

    continue;
}
```

**狀態機介入點**：
1. 控制結構開始時，合併 pending Comment/Code
2. 控制結構結束時，結果變成 pending（可吸收尾部空行）

#### [5] 空行處理

```rust
if trimmed.is_empty() {
    let blank_entry = Entry::new(EntryType::Code, "L{}", "")
        .with_line_number(line_number)
        .with_raw_line(line);

    // ★ 狀態機介入
    match &mut pending_entry {
        Some(pending) if pending.entry_type == Comment => {
            pending.merge_trailing(blank_entry);  // Comment 吸收空行
        }
        Some(pending) if pending.is_blank() => {
            pending.merge_trailing(blank_entry);  // 空行合併空行
        }
        Some(pending) if pending.entry_type == Code => {
            pending.merge_trailing(blank_entry);  // Code 吸收尾部空行
        }
        Some(_) => {
            result.add_entry(pending_entry.take());
            pending_entry = Some(blank_entry);
        }
        None => {
            pending_entry = Some(blank_entry);
        }
    }
    continue;
}
```

**狀態機介入**：完全由狀態機控制

#### [6] Comment 處理

```rust
if is_standalone_comment(trimmed) {
    let comment_entry = Entry::new(EntryType::Comment, "#L{}", line)
        .with_line_number(line_number)
        .with_raw_line(line);

    // ★ 狀態機介入
    match &mut pending_entry {
        Some(pending) if pending.entry_type == Comment => {
            pending.merge_trailing(comment_entry);  // Comment 合併 Comment
        }
        Some(_) => {
            result.add_entry(pending_entry.take());
            pending_entry = Some(comment_entry);
        }
        None => {
            pending_entry = Some(comment_entry);
        }
    }
    continue;
}
```

**狀態機介入**：完全由狀態機控制

#### [7-10] Structured Entry 處理

```rust
// Alias, EnvVar, Source, Function
match try_parse_alias(trimmed, line_number) {
    SingleLine(mut entry) => {
        // ★ 狀態機介入：flush pending
        if let Some(pending) = pending_entry.take() {
            result.add_entry(pending);
        }

        // 關聯前面的 Comment
        if let Some(comment) = pending_comment.take() {
            entry = entry.with_comment(comment);
        }

        result.add_entry(entry);
        continue;
    }
    MultiLineStart { builder } => {
        // ★ 狀態機介入：flush pending
        if let Some(pending) = pending_entry.take() {
            result.add_entry(pending);
        }
        current_alias = Some(builder);
        continue;
    }
    NotAlias => {}
}
```

**狀態機介入**：遇到 structured entry 時 flush pending

#### [11] Fallback: Code

```rust
let code_entry = Entry::new(EntryType::Code, "L{}", line)
    .with_line_number(line_number)
    .with_raw_line(line);

// ★ 狀態機介入
match &mut pending_entry {
    Some(pending) if pending.entry_type == Comment => {
        // Comment + Code → 類型升級，繼續 pending
        pending.merge_trailing(code_entry);
    }
    Some(pending) if pending.entry_type == Code => {
        // Code + Code → 不合併，輸出舊的
        result.add_entry(pending_entry.take());
        pending_entry = Some(code_entry);
    }
    Some(_) => {
        result.add_entry(pending_entry.take());
        pending_entry = Some(code_entry);
    }
    None => {
        pending_entry = Some(code_entry);
    }
}
```

**狀態機介入**：完全由狀態機控制

### 1.5 結束處理

```rust
// 迴圈結束後

// 1. Flush 殘留的 pending entry
if let Some(entry) = pending_entry.take() {
    result.add_entry(entry);
}

// 2. 警告未閉合的多行結構
if in_function {
    result.add_warning("Unclosed function definition");
}
if current_alias.is_some() {
    result.add_warning("Unclosed multi-line alias");
}
if current_env.is_some() {
    result.add_warning("Unclosed multi-line export");
}
```

### 1.6 Pending 狀態機介入時機總結

| 處理階段 | 狀態機介入 | 說明 |
|----------|-----------|------|
| 多行函數 | ❌ | 函數內部不處理 pending |
| 多行 Alias/EnvVar | ❌ | builder 模式，不處理 pending |
| 控制結構開始 | ✅ 合併 | pending Comment/Code 合併到 block 開頭 |
| 控制結構結束 | ✅ 變成 pending | 不直接輸出，等待吸收空行 |
| 空行 | ✅ 完全控制 | 根據 pending 類型決定合併或新建 |
| Comment | ✅ 完全控制 | 根據 pending 類型決定合併或新建 |
| Structured Entry | ✅ flush | 輸出 pending 後處理 structured entry |
| Fallback Code | ✅ 完全控制 | 根據 pending 類型決定合併或新建 |
| 結束 | ✅ flush | 輸出殘留的 pending |

---

## 2. Format 輸出流程

### 2.1 流程概覽

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          Formatter 完整流程                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  輸入: entries: &[Entry], config: &Config                               │
│          │                                                               │
│          ▼                                                               │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │               檢查 config.format.group_by_type                   │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│          │                                                               │
│     ┌────┴────┐                                                         │
│     │         │                                                         │
│   false      true                                                       │
│     │         │                                                         │
│     ▼         ▼                                                         │
│  ┌──────────────┐     ┌──────────────────────────────────────────┐     │
│  │ 原始順序輸出  │     │            分組排序輸出                    │     │
│  │              │     │                                          │     │
│  │ • 按行號排序 │     │ • 找出 attached comments                 │     │
│  │ • 逐個輸出   │     │ • 分組 Alias/EnvVar/Source/Function     │     │
│  │              │     │ • 各組內排序（字母/依賴）                 │     │
│  │              │     │ • Comment/Code 保持原位                  │     │
│  │              │     │ • 第一個 entry 時輸出整組                 │     │
│  └──────────────┘     └──────────────────────────────────────────┘     │
│          │                     │                                        │
│          └──────────┬──────────┘                                        │
│                     ▼                                                    │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                      format_entry()                              │    │
│  │                                                                  │    │
│  │  • Alias/EnvVar/Source: raw_line 優先，否則重新格式化           │    │
│  │  • Function: 使用 raw_line 但重新縮排                           │    │
│  │  • Code/Comment: 直接使用 raw_line                              │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                     │                                                    │
│                     ▼                                                    │
│  輸出: String (formatted content)                                       │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.2 原始順序輸出（group_by_type = false）

```rust
fn format(&self, entries: &[Entry], config: &Config) -> String {
    let mut output = String::new();

    // 1. 按行號排序
    let mut sorted_entries: Vec<_> = entries.iter().collect();
    sorted_entries.sort_by_key(|e| e.line_number.unwrap_or(0));

    // 2. 逐個輸出
    for entry in sorted_entries {
        if entry.entry_type == Code && entry.value.is_empty() {
            // 空行處理：根據行範圍輸出正確數量的換行
            let count = entry.end_line.unwrap_or(start) - start + 1;
            for _ in 0..count {
                output.push('\n');
            }
        } else {
            output.push_str(&self.format_entry(entry));
            output.push('\n');
        }
    }

    output
}
```

**流程圖**：

```
entries ──→ 按 line_number 排序 ──→ for each entry:
                                          │
                                          ├─ Code(blank)? → 輸出 N 個 \n
                                          │
                                          └─ 其他 → format_entry() + \n
```

### 2.3 分組排序輸出（group_by_type = true）

```rust
fn format(&self, entries: &[Entry], config: &Config) -> String {
    // 1. 找出 attached comments
    let attached_comments = self.find_attached_comments(entries);

    // 2. 分組 structured entries
    let mut grouped: HashMap<EntryType, Vec<&Entry>> = HashMap::new();
    for entry in entries {
        match entry.entry_type {
            Alias | EnvVar | Source | Function => {
                grouped.entry(entry.entry_type).or_default().push(entry);
            }
            _ => {}  // Comment/Code 不分組
        }
    }

    // 3. 組內排序
    for (entry_type, type_entries) in grouped.iter_mut() {
        if config.format.sort_alphabetically {
            if entry_type == EnvVar {
                // EnvVar 使用拓撲排序（依賴順序）
                *type_entries = topological_sort(type_entries, true);
            } else {
                // 其他類型字母排序
                type_entries.sort_by(|a, b| a.name.cmp(&b.name));
            }
        }
    }

    // 4. 按原始行號順序遍歷
    let mut sorted_entries: Vec<_> = entries.iter().collect();
    sorted_entries.sort_by_key(|e| e.line_number.unwrap_or(0));

    let mut output_types: HashSet<EntryType> = HashSet::new();

    for entry in sorted_entries {
        match entry.entry_type {
            Code | Comment => {
                // 跳過 attached comments（稍後跟隨 entry 輸出）
                if is_attached(entry, &attached_comments) {
                    continue;
                }
                output_entry(entry);
            }
            entry_type @ (Alias | EnvVar | Source | Function) => {
                // 只在第一次遇到該類型時輸出整組
                if is_first_of_type(entry, &output_types) {
                    output_types.insert(entry_type);

                    for grouped_entry in grouped.get(&entry_type) {
                        // 先輸出 attached comments
                        output_attached_comments(grouped_entry);
                        // 再輸出 entry
                        output_entry(grouped_entry);
                    }
                }
            }
        }
    }

    output
}
```

**流程圖**：

```
entries
    │
    ├──→ find_attached_comments() ──→ attached_comments map
    │
    ├──→ 分組 Alias/EnvVar/Source/Function ──→ grouped map
    │
    ├──→ 組內排序（字母/拓撲）
    │
    └──→ 按 line_number 遍歷:
              │
              ├─ Comment/Code:
              │     ├─ attached? → skip
              │     └─ 否則 → 輸出
              │
              └─ Alias/EnvVar/Source/Function:
                    ├─ 已輸出過該類型? → skip
                    └─ 第一次 → 輸出整組（含 attached comments）
```

### 2.4 format_entry() 處理邏輯

```rust
fn format_entry(&self, entry: &Entry) -> String {
    match entry.entry_type {
        // Alias/EnvVar/Source: 優先使用 raw_line
        Alias => {
            if let Some(ref raw) = entry.raw_line {
                return raw.clone();  // 未編輯：原樣輸出
            }
            self.format_alias(entry)  // 已編輯：重新格式化
        }

        EnvVar => {
            if let Some(ref raw) = entry.raw_line {
                return raw.clone();
            }
            self.format_export(entry)
        }

        Source => {
            if let Some(ref raw) = entry.raw_line {
                return raw.clone();
            }
            self.format_source(entry)
        }

        // Function: 使用 raw_line 但重新格式化縮排
        Function => self.format_function(entry, &self.indent_style),

        // Code/Comment: 永遠使用 raw_line
        Code | Comment => {
            entry.raw_line.clone().unwrap_or(entry.value.clone())
        }
    }
}
```

**決策流程圖**：

```
format_entry(entry)
        │
        ├─ Alias ───────┐
        ├─ EnvVar ──────┼──→ raw_line 存在? ──┬─ Yes → return raw_line
        ├─ Source ──────┘                     └─ No  → format_xxx()
        │
        ├─ Function ────────→ format_function() (使用 raw_line 但重新縮排)
        │
        └─ Code/Comment ────→ raw_line.unwrap_or(value)
```

### 2.5 各類型格式化方法

#### format_alias()

```rust
fn format_alias(&self, entry: &Entry) -> String {
    let value = &entry.value;

    if value.contains('\n') {
        // 多行值
        if !value.contains('\'') {
            format!("alias {}='{}'", entry.name, value)
        } else {
            // 值含單引號，使用雙引號並轉義
            let escaped = escape_for_double_quotes(value);
            format!("alias {}=\"{}\"", entry.name, escaped)
        }
    } else {
        // 單行值
        if needs_quotes(value) {
            if value.contains('\'') {
                format!("alias {}=\"{}\"", entry.name, escape_double_quotes(value))
            } else {
                format!("alias {}='{}'", entry.name, value)
            }
        } else {
            format!("alias {}='{}'", entry.name, value)
        }
    }
}
```

#### format_export()

```rust
fn format_export(&self, entry: &Entry) -> String {
    let value = &entry.value;

    if value.contains('\n') {
        // 多行值：使用雙引號
        let escaped = escape_for_double_quotes(value);
        format!("export {}=\"{}\"", entry.name, escaped)
    } else {
        // 單行值
        if value.contains(' ') || value.contains('$') {
            format!("export {}=\"{}\"", entry.name, value)
        } else {
            format!("export {}={}", entry.name, value)
        }
    }
}
```

#### format_source()

```rust
fn format_source(&self, entry: &Entry) -> String {
    format!("source {}", entry.value)
}
```

#### format_function()

```rust
fn format_function(&self, entry: &Entry, indent_style: &str) -> String {
    if let Some(ref raw) = entry.raw_line {
        // 有 raw_line：重新格式化縮排
        return self.format_raw_function(raw, indent_style);
    }

    // 無 raw_line：從 value (body) 重建
    let body = format_body_preserve_relative(&entry.value, indent_style);
    format!("{}() {{\n{}\n}}", entry.name, body)
}

fn format_raw_function(&self, raw: &str, indent_style: &str) -> String {
    let lines: Vec<&str> = raw.lines().collect();

    if lines.len() <= 2 {
        return raw.to_string();  // 單行或最小函數，原樣返回
    }

    // 提取 body（第一行和最後一行之間）
    let body = lines[1..lines.len()-1].join("\n");
    let formatted_body = format_body_preserve_relative(&body, indent_style);

    format!("{}\n{}\n{}", lines[0], formatted_body, lines.last().unwrap())
}
```

### 2.6 空行處理

```rust
// 空行 entry 特殊處理
if entry.entry_type == EntryType::Code && entry.value.is_empty() {
    // 根據 line_number 和 end_line 計算空行數量
    if let (Some(start), Some(end)) = (entry.line_number, entry.end_line) {
        let count = end - start + 1;
        for _ in 0..count {
            output.push('\n');
        }
    } else {
        output.push('\n');
    }
} else {
    output.push_str(&self.format_entry(entry));
    output.push('\n');
}
```

**範例**：
- `Code(L5, blank)` → 1 個 `\n`
- `Code(L5-L7, blank)` → 3 個 `\n`

---

## 3. Parse → Format 完整資料流

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         完整資料流                                       │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  原始檔案 (String)                                                       │
│       │                                                                  │
│       ▼                                                                  │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │                         Parser                                    │   │
│  │                                                                   │   │
│  │  • 逐行處理                                                       │   │
│  │  • 多行 Builder 累積                                              │   │
│  │  • Pending 狀態機合併 Comment/Code                               │   │
│  │  • 產生 Entry（含 raw_line）                                      │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│       │                                                                  │
│       ▼                                                                  │
│  ParseResult {                                                           │
│      entries: Vec<Entry>,    ← 每個 entry 都有 raw_line                 │
│      warnings: Vec<Warning>                                              │
│  }                                                                       │
│       │                                                                  │
│       │                ┌──────────────────┐                             │
│       │                │    TUI 編輯      │                             │
│       │                │                  │                             │
│       ├───────────────→│  • 顯示 entries  │                             │
│       │                │  • 編輯 value    │                             │
│       │                │  • 新 entry      │                             │
│       │                │    無 raw_line   │                             │
│       │                └────────┬─────────┘                             │
│       │                         │                                        │
│       │    ┌────────────────────┘                                       │
│       │    │                                                             │
│       ▼    ▼                                                             │
│  entries: Vec<Entry>                                                     │
│  (有些有 raw_line，有些沒有)                                             │
│       │                                                                  │
│       ▼                                                                  │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │                        Formatter                                  │   │
│  │                                                                   │   │
│  │  for each entry:                                                  │   │
│  │    ├─ raw_line 存在? → 使用 raw_line（保留原始格式）              │   │
│  │    └─ raw_line 為 None? → 使用 format_xxx()（重新格式化）         │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│       │                                                                  │
│       ▼                                                                  │
│  輸出檔案 (String)                                                       │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 4. 程式碼位置

| 功能 | 檔案位置 |
|------|----------|
| Parser 主邏輯 | `src/parser/bash/mod.rs:92-481` |
| Formatter 主邏輯 | `src/formatter/bash.rs:173-318` |
| format_entry() | `src/formatter/bash.rs:320-351` |
| format_alias() | `src/formatter/bash.rs:75-103` |
| format_export() | `src/formatter/bash.rs:105-125` |
| format_function() | `src/formatter/bash.rs:132-164` |
| Pending 狀態機 | `src/parser/bash/mod.rs:239-443` |
| merge_trailing() | `src/model/entry.rs:90-136` |
