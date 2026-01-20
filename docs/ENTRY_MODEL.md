# Entry 資料模型與 Parser/Formatter 邏輯

本文件詳細說明 wenv 的 Entry 資料結構、各類型 Entry 的特性，以及 Parser 和 Formatter 的處理邏輯。

---

## 1. Entry 結構定義

```rust
pub struct Entry {
    pub entry_type: EntryType,      // 類型：Alias, Function, EnvVar, Source, Code, Comment
    pub name: String,               // 識別符（名稱或行號）
    pub value: String,              // 處理後的值
    pub line_number: Option<usize>, // 起始行號（1-based）
    pub end_line: Option<usize>,    // 結束行號（用於多行 entry）
    pub comment: Option<String>,    // 附加註解（較少使用）
    pub raw_line: Option<String>,   // 原始完整內容（所有行以 \n 連接）
}
```

### 欄位說明

| 欄位 | 用途 | 範例 |
|------|------|------|
| `entry_type` | Entry 類型 | `EntryType::Alias` |
| `name` | 識別符 | Alias: `ll`, Function: `greet`, Code: `L5-L10` |
| `value` | 處理後的值（去除封裝） | Alias: `ls -la`（無 `alias ll='`） |
| `line_number` | 起始行號 | `Some(5)` |
| `end_line` | 結束行號 | `Some(10)` 或與 line_number 相同 |
| `comment` | 附加註解 | 較少使用，raw_line 已包含完整內容 |
| `raw_line` | **原始完整內容** | `alias ll='ls -la'`（完整原始語法） |

### raw_line 格式規範

- **用途**：保存原始檔案內容，用於未編輯 entry 的還原輸出
- **多行格式**：N 行使用 N-1 個 `\n` 連接（分隔符，非終止符）
- **範例**：3 行 entry 存為 `"line1\nline2\nline3"`
- **分割方式**：使用 `raw.split('\n')` 而非 `lines()` 或其他方法

---

## 2. Entry 基本特性表

| 類型 | 支援多行 | 多行判斷機制 | Parse: value 處理 | Format: 封裝方式 | raw_line 優先 |
|------|----------|--------------|-------------------|------------------|---------------|
| **Alias** | ✅ | 單引號計數（奇數=未閉合） | 去除 `alias name='` 和 `'` | 依值選擇單/雙引號 | ✅ 未編輯時使用 |
| **EnvVar** | ✅ | 單引號計數（奇數=未閉合） | 去除 `export VAR=` 和引號 | 依值選擇是否加引號 | ✅ 未編輯時使用 |
| **Source** | ❌ | 不支援多行 | 去除 `source ` 或 `. ` 前綴 | 加上 `source ` 前綴 | ✅ 未編輯時使用 |
| **Function** | ✅ | 大括號計數（開=+1, 閉=-1, 歸零=結束） | 提取 body（不含 `{` `}`） | 重新格式化縮排 | ✅ 但會重新縮排 |
| **Code** | ✅ | 控制結構深度計數 + pending 狀態機 | 保留原始內容 | 直接輸出 raw_line | ✅ 永遠使用 |
| **Comment** | ✅ | pending 狀態機合併相鄰行 | 保留原始內容（含 `#`） | 直接輸出 raw_line | ✅ 永遠使用 |

---

## 2.1 多行判斷機制詳解

### Alias / EnvVar：單引號計數

```rust
// QuotedValueBuilder::has_unclosed_single_quote()
// 計算行內單引號數量（排除雙引號內的單引號）
// 奇數 = 未閉合，需繼續累積下一行

// 範例：
"alias multi='line1"     // 1 個單引號 → 未閉合，開始多行
"line2"                  // 0 個單引號 → 繼續累積
"line3'"                 // 1 個單引號 → 總計偶數，閉合
```

**完整性檢查**：
```rust
fn is_complete(&self) -> bool {
    let total_quotes: usize = self.lines.iter()
        .map(|l| Self::count_single_quotes(l))
        .sum();
    total_quotes % 2 == 0  // 偶數 = 完整
}
```

### Function：大括號計數

```rust
// 偵測函數開始：detect_function_start()
// 匹配 "name() {" 或 "function name {"

// 追蹤函數邊界：
let (open, close) = count_braces_outside_quotes(line);
brace_count += open;      // { 增加計數
brace_count -= close;     // } 減少計數

if brace_count == 0 {
    // 函數結束
}
```

**範例**：
```bash
greet() {              # brace_count = 1
    if true; then      # brace_count = 1 (不計 if 的結構)
        echo "hi"
    fi
}                      # brace_count = 0 → 函數結束
```

### Code：控制結構深度 + pending 狀態機

**控制結構深度計數**：
```rust
// count_control_start(): if, while, for, until, case, select → +1
// count_control_end(): fi, done, esac → -1

control_depth -= count_control_end(line);   // 先減
control_depth += count_control_start(line); // 後加

// 深度 > 0 時，所有行累積到 CodeBlockBuilder
// 深度 = 0 且剛離開控制結構 → 控制塊結束
```

**pending 狀態機**：
```rust
// 控制塊結束後，不立即輸出，而是變成 pending
// 這樣可以吸收後續的空行

if prev_depth > 0 && control_depth == 0 {
    pending_entry = Some(block.build());  // 變成 pending
}
```

### Comment：pending 狀態機合併

```rust
// 每行 Comment 不立即輸出，而是變成 pending
// 下一行根據類型決定是否合併

match &mut pending_entry {
    Some(pending) if pending.entry_type == EntryType::Comment => {
        // Comment + Comment → 合併
        pending.merge_trailing(comment_entry);
    }
    Some(pending) if pending.entry_type == EntryType::Comment => {
        // Comment + blank → 吸收空行
        pending.merge_trailing(blank_entry);
    }
    Some(pending) if pending.entry_type == EntryType::Comment => {
        // Comment + non-blank Code → 類型升級為 Code
        pending.merge_trailing(code_entry);
    }
}
```

---

## 2.2 行號計算機制

### 通用規則

| 欄位 | 計算方式 |
|------|----------|
| `line_number` | entry 起始行（1-based） |
| `end_line` | entry 結束行（單行時 = line_number） |
| `name` | 依類型：識別符 或 `L{line}` 或 `#L{line}` |

### 各類型行號來源

#### Alias / EnvVar / Source（單行）

```rust
// 直接使用當前行號
Entry::new(EntryType::Alias, name, value)
    .with_line_number(line_number)   // 當前行
    .with_raw_line(line)
// 單行 entry 不設定 end_line（或 end_line = line_number）
```

#### Alias / EnvVar（多行，QuotedValueBuilder）

```rust
// start_line 記錄在 builder 建立時
let builder = QuotedValueBuilder::new(name, line_num, first_line);

// end_line 在 build() 時計算
fn build(self) -> Entry {
    let end_line = self.start_line + self.lines.len() - 1;
    Entry::new(...)
        .with_line_number(self.start_line)
        .with_end_line(end_line)
}
```

#### Function（FunctionBuilder）

```rust
// 函數開始時記錄 start_line
current_func = Some(FunctionBuilder::new(func_name, line_number));

// 每行加入 builder
func.add_line(line);

// build() 時計算 end_line
fn build(self) -> Entry {
    let end_line = self.start_line + self.lines.len() - 1;
    Entry::new(...)
        .with_line_number(self.start_line)
        .with_end_line(end_line)
}
```

#### Code（CodeBlockBuilder，控制結構）

```rust
// 控制結構開始時記錄
let mut block = CodeBlockBuilder::new(line_number);

// 或從 pending entry 繼承（Comment + 控制結構合併）
let start_line = pending.line_number.unwrap_or(line_number);
let mut block = CodeBlockBuilder::new(start_line);

// 每行加入
block.add_line(line);

// build() 計算 end_line
fn build(self) -> Entry {
    let end_line = self.start_line + self.lines.len() - 1;
    Entry::new(EntryType::Code, name, body)
        .with_line_number(self.start_line)
        .with_end_line(end_line)
}
```

#### Code / Comment（pending 狀態機合併）

```rust
// 合併時更新 end_line
fn merge_trailing(&mut self, other: Entry) {
    self.end_line = other.end_line.or(other.line_number);

    // 更新 name 反映新行範圍
    if let (Some(start), Some(end)) = (self.line_number, self.end_line) {
        if start == end {
            self.name = format!("L{}", start);    // 或 "#L{}"
        } else {
            self.name = format!("L{}-L{}", start, end);
        }
    }
}
```

### name 命名規則

| 類型 | name 格式 | 範例 |
|------|-----------|------|
| Alias | 別名名稱 | `ll`, `gs` |
| EnvVar | 變數名稱 | `PATH`, `EDITOR` |
| Function | 函數名稱 | `greet`, `deploy` |
| Source | `L{行號}` | `L15` |
| Comment (單行) | `#L{行號}` | `#L5` |
| Comment (多行) | `#L{起始}-L{結束}` | `#L5-L8` |
| Code (單行) | `L{行號}` | `L10` |
| Code (多行) | `L{起始}-L{結束}` | `L10-L25` |

### 行號計算流程圖

```
┌─────────────────────────────────────────────────────────────────────┐
│                         行號計算流程                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  for (line_num, line) in content.lines().enumerate() {              │
│      let line_number = line_num + 1;  // 0-based → 1-based          │
│                                                                      │
│      ┌─────────────────────────────────────────────────────────┐    │
│      │ 單行 Entry (Alias/EnvVar/Source/Comment/Code)            │    │
│      │   line_number = 當前行號                                  │    │
│      │   end_line = None 或 = line_number                       │    │
│      └─────────────────────────────────────────────────────────┘    │
│                                                                      │
│      ┌─────────────────────────────────────────────────────────┐    │
│      │ 多行 Entry 開始 (Builder 建立)                            │    │
│      │   builder.start_line = 當前行號                          │    │
│      │   builder.lines.push(line)                               │    │
│      └─────────────────────────────────────────────────────────┘    │
│                          │                                           │
│                          ▼                                           │
│      ┌─────────────────────────────────────────────────────────┐    │
│      │ 多行 Entry 持續 (每行)                                    │    │
│      │   builder.lines.push(line)                               │    │
│      └─────────────────────────────────────────────────────────┘    │
│                          │                                           │
│                          ▼                                           │
│      ┌─────────────────────────────────────────────────────────┐    │
│      │ 多行 Entry 結束 (Builder.build())                         │    │
│      │   end_line = start_line + lines.len() - 1                │    │
│      └─────────────────────────────────────────────────────────┘    │
│                                                                      │
│      ┌─────────────────────────────────────────────────────────┐    │
│      │ Pending 合併 (merge_trailing)                            │    │
│      │   end_line = other.end_line 或 other.line_number         │    │
│      │   name 更新為 "L{start}-L{end}"                          │    │
│      └─────────────────────────────────────────────────────────┘    │
│  }                                                                   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 圖示：value vs raw_line

```
原始檔案:  alias ll='ls -la'
              │
          [Parser]
              │
              ▼
Entry {
  name:     "ll"                    ← 提取名稱
  value:    "ls -la"                ← 去除封裝，純值
  raw_line: "alias ll='ls -la'"     ← 保留原始
}
              │
          [Formatter]
              │
     ┌────────┴────────┐
     │                 │
 [未編輯]           [已編輯]
     │                 │
 使用 raw_line      使用 format_alias()
     │                 │
     ▼                 ▼
"alias ll='ls -la'"  "alias ll='new value'"
```

---

## 3. Parser 邏輯詳解

### 3.1 Alias

**檔案**：`parser/bash/parsers.rs:try_parse_alias()`

**匹配模式**：
1. 單引號：`alias name='value'`
2. 雙引號：`alias name="value"`
3. 無引號：`alias name=value`
4. 多行開始：`alias name='unclosed...`

**處理邏輯**：
```rust
// 單行完整 alias
if let Some(caps) = ALIAS_SINGLE_RE.captures(line) {
    Entry::new(EntryType::Alias, caps[1], caps[2])  // name, value (已去引號)
        .with_raw_line(line)                         // 保留原始
}

// 多行 alias（單引號未閉合）
if QuotedValueBuilder::has_unclosed_single_quote(line) {
    // 啟動 QuotedValueBuilder，持續累積直到引號閉合
}
```

**value 處理**：
- 提取 `=` 後的值
- 去除外層引號（`'value'` → `value`）
- 多行時合併所有行並去除首尾引號

**raw_line 內容**：完整原始語法，包含 `alias name='value'`

---

### 3.2 EnvVar (Export)

**檔案**：`parser/bash/parsers.rs:try_parse_export()`

**匹配模式**：
1. 單行：`export VAR=value` 或 `export VAR="value"`
2. 多行開始：`export VAR='unclosed...`

**處理邏輯**：
```rust
if let Some(caps) = EXPORT_RE.captures(line) {
    let (value_clean, _comment) = extract_comment(&caps[2], '#');
    let value = strip_quotes(&value_clean);  // 去除引號
    Entry::new(EntryType::EnvVar, caps[1], value)
        .with_raw_line(line)
}
```

**value 處理**：
- 去除 `export VAR=` 前綴
- 去除外層引號
- 去除行尾註解

**raw_line 內容**：完整原始語法

---

### 3.3 Source

**檔案**：`parser/bash/parsers.rs:try_parse_source()`

**匹配模式**：
- `source file`
- `. file`

**處理邏輯**：
```rust
if let Some(caps) = SOURCE_RE.captures(line) {
    let path = strip_quotes(&caps[1]);
    let name = format!("L{}", line_num);  // 使用行號作為名稱
    Entry::new(EntryType::Source, name, path)
        .with_raw_line(line)
}
```

**value 處理**：只保留路徑，去除 `source ` 前綴

**raw_line 內容**：完整原始語法

---

### 3.4 Function

**檔案**：`parser/builders/function.rs:FunctionBuilder`

**匹配模式**：
- `name() {`
- `function name() {`
- `function name {`

**處理邏輯**：
```rust
// 偵測函數開始
if detect_function_start(line).is_some() {
    // 啟動 FunctionBuilder
    // 使用大括號計數追蹤函數邊界
}

// FunctionBuilder::build()
fn build(self) -> Entry {
    let body = self.extract_body();  // 提取 body（不含開頭結尾行）
    let raw = self.lines.join("\n"); // 完整內容
    Entry::new(EntryType::Function, self.name, body)
        .with_raw_line(raw)
}
```

**value 處理**：
- 只保留函數 body（中間行）
- 不含 `name() {` 開頭行
- 不含 `}` 結尾行

**raw_line 內容**：完整函數定義

---

### 3.5 Comment

**檔案**：`parser/builders/comment.rs:CommentBlockBuilder`

**匹配模式**：以 `#` 開頭的行（trimmed）

**處理邏輯**：
```rust
// 相鄰註解行合併
fn build(self) -> Entry {
    let raw = self.lines.join("\n");
    let value = raw.clone();  // value 保留完整原始內容（含 #）
    Entry::new(EntryType::Comment, name, value)
        .with_raw_line(raw)
}
```

**value 處理**：保留完整原始內容（含 `#` 前綴）

**raw_line 內容**：與 value 相同

**合併規則**（pending entry 狀態機）：

| 場景 | 結果 |
|------|------|
| Comment + Comment | Comment（合併） |
| Comment + blank | Comment（吸收空行） |
| Comment + non-blank Code | **Code**（類型升級） |
| Comment + control structure | **Code**（合併到控制塊） |

---

### 3.6 Code

**檔案**：`parser/builders/code_block.rs:CodeBlockBuilder`

**匹配模式**：
- 控制結構：`if`/`fi`, `while`/`done`, `for`/`done`, `case`/`esac`
- 空行
- 其他非結構化程式碼

**處理邏輯**：
```rust
// 控制結構
fn build(self) -> Entry {
    let body = self.lines.join("\n");
    Entry::new(EntryType::Code, name, body.clone())
        .with_raw_line(body)
}

// 空行
fn create_blank_line_entry(start, end) -> Entry {
    Entry::new(EntryType::Code, name, String::new())
        .with_raw_line(String::new())
}
```

**value 處理**：保留完整原始內容

**raw_line 內容**：與 value 相同

---

## 4. Formatter 邏輯詳解

### 4.1 format_entry() 主邏輯

**檔案**：`formatter/bash.rs:format_entry()`

**核心原則**：優先使用 `raw_line`（未編輯 entry），否則重新格式化

```rust
fn format_entry(&self, entry: &Entry) -> String {
    match entry.entry_type {
        EntryType::Alias => {
            if let Some(ref raw) = entry.raw_line {
                return raw.clone();  // 未編輯：使用 raw_line
            }
            self.format_alias(entry)  // 已編輯：重新格式化
        }
        EntryType::EnvVar => { /* 同上 */ }
        EntryType::Source => { /* 同上 */ }
        EntryType::Function => self.format_function(entry),  // 總是重新縮排
        EntryType::Code | EntryType::Comment => {
            entry.raw_line.clone().unwrap_or(entry.value.clone())
        }
    }
}
```

---

### 4.2 format_alias()

**觸發條件**：`entry.raw_line` 為 `None`（已編輯的 entry）

**邏輯**：
```rust
fn format_alias(&self, entry: &Entry) -> String {
    let value = &entry.value;

    // 多行值處理
    if value.contains('\n') {
        if !value.contains('\'') {
            // 無單引號：使用單引號包裹
            format!("alias {}='{}'", entry.name, value)
        } else {
            // 有單引號：使用雙引號，需轉義 \, ", $, `
            let escaped = value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('$', "\\$")
                .replace('`', "\\`");
            format!("alias {}=\"{}\"", entry.name, escaped)
        }
    } else {
        // 單行值：依內容選擇引號
        if value.contains(' ') || value.contains('$') || value.contains('"') {
            if value.contains('\'') {
                format!("alias {}=\"{}\"", entry.name, value.replace('"', "\\\""))
            } else {
                format!("alias {}='{}'", entry.name, value)
            }
        } else {
            format!("alias {}='{}'", entry.name, value)
        }
    }
}
```

---

### 4.3 format_export()

**觸發條件**：`entry.raw_line` 為 `None`

**邏輯**：
```rust
fn format_export(&self, entry: &Entry) -> String {
    let value = &entry.value;

    // 多行值：使用雙引號
    if value.contains('\n') {
        let escaped = value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`");
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

---

### 4.4 format_source()

**邏輯**：
```rust
fn format_source(&self, entry: &Entry) -> String {
    format!("source {}", entry.value)
}
```

---

### 4.5 format_function()

**邏輯**：總是使用 `raw_line` 但重新格式化縮排

```rust
fn format_function(&self, entry: &Entry, indent_style: &str) -> String {
    if let Some(ref raw) = entry.raw_line {
        return self.format_raw_function(raw, indent_style);
    }

    // 無 raw_line：從 value (body) 重建
    let body = format_body_preserve_relative(&entry.value, indent_style);
    format!("{}() {{\n{}\n}}", entry.name, body)
}
```

---

### 4.6 Comment/Code

**邏輯**：直接使用 `raw_line`

```rust
EntryType::Code | EntryType::Comment => {
    entry.raw_line.clone().unwrap_or(entry.value.clone())
}
```

---

## 5. TUI 編輯流程

### 5.1 開始編輯

```rust
fn start_editing(&mut self) {
    let value = if matches!(entry_type, EntryType::Comment | EntryType::Code) {
        entry.raw_line.clone().unwrap_or(entry.value.clone())  // 使用 raw_line
    } else {
        entry.value.clone()  // 使用 value（純值）
    };

    self.edit_state = Some(EditState {
        value_buffer: value,  // 編輯緩衝區
        ...
    });
}
```

### 5.2 儲存編輯

```rust
fn save_edit(&mut self) {
    // 建立新 Entry（無 raw_line）
    let entry = Entry::new(
        state.entry_type,
        state.name_buffer.trim().to_string(),
        state.value_buffer.clone(),
    );
    // Entry::new() 設定 raw_line: None

    // Formatter 會調用 format_alias() 等方法重新格式化
    formatter.format_entry(&entry)
}
```

### 5.3 Comment/Code 特殊處理

```rust
// Comment/Code 使用 replace_line_range() 直接替換
if matches!(state.entry_type, EntryType::Comment | EntryType::Code) {
    let new_content = self.replace_line_range(
        &content,
        start_line,
        end_line,
        &state.value_buffer,  // 直接使用 value_buffer
    );
}
```

---

## 6. 資料流總結

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              完整資料流                                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  原始檔案                     Parser                      Entry          │
│  ─────────                   ──────                      ─────          │
│  alias ll='ls -la'    →    提取 name, value       →   name: "ll"       │
│                              去除封裝                   value: "ls -la" │
│                              保留 raw_line              raw_line: 完整  │
│                                                                          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│               ┌────────────────┬────────────────┐                       │
│               │                │                │                       │
│           [未編輯]          [TUI編輯]        [新增]                      │
│               │                │                │                       │
│        raw_line 存在      Entry::new()     Entry::new()                 │
│               │           raw_line: None   raw_line: None               │
│               │                │                │                       │
│               ▼                ▼                ▼                       │
│           Formatter         Formatter        Formatter                  │
│               │                │                │                       │
│         使用 raw_line    使用 format_*()   使用 format_*()              │
│               │                │                │                       │
│               ▼                ▼                ▼                       │
│           原始格式          重新格式化       正確封裝                    │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 7. 常見問題

### Q1: 為什麼 Alias/EnvVar 要去除引號存到 value？

**A**: TUI 編輯時只顯示純值，用戶不需要處理引號。格式化時由 Formatter 根據值的內容自動選擇適當的引號方式。

### Q2: raw_line 什麼時候會是 None？

**A**: 兩種情況：
1. 透過 TUI 編輯後的 entry（使用 `Entry::new()` 建立）
2. 透過 API 新建的 entry

### Q3: Comment/Code 為什麼不需要重新格式化？

**A**: Comment 和 Code 沒有固定語法結構，應該保留原始格式（縮排、空行等）。

### Q4: 多行 Alias 如何處理？

**A**:
- **Parse**：使用 `QuotedValueBuilder` 追蹤單引號配對
- **Format**：優先使用單引號；若值含單引號則使用雙引號並轉義
