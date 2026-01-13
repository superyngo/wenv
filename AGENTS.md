# AGENTS.md

Instructions for AI coding agents working in this repository.

## Project Overview

**wenv** is a cross-platform CLI tool for managing shell configuration files (`.bashrc`, PowerShell profiles). It parses, organizes, edits, and maintains aliases, functions, environment variables, and source statements.

## Build/Lint/Test Commands

```bash
# Build
cargo build              # Debug build
cargo build --release    # Release build
cargo check              # Fast syntax/type check (use frequently)

# Lint and Format
cargo fmt                # Format code (run before commits)
cargo clippy             # Lint with clippy (fix all warnings)
cargo fmt -- --check     # Check formatting without modifying

# Run Tests
cargo test               # Run all tests
cargo test --lib         # Library unit tests only
cargo test --test integration  # Integration tests only

# Run a Single Test
cargo test test_parse_alias_single_quote           # Run test by name
cargo test bash::tests::test_parse_alias           # Run by module path
cargo test bash_tests -- --nocapture               # Run with output

# Run Tests in Specific Module
cargo test parser::bash::tests                     # All tests in bash parser
cargo test checker::                               # All checker tests

# Run CLI
cargo run -- list                    # Run with subcommand
cargo run -- --file ~/.bashrc list   # Specify config file
cargo run -- --help                  # Show help
```

## Architecture

### Trait-Based Design

Core traits for shell extensibility:

- **`Parser`** (`src/parser/mod.rs`) - Implemented by `BashParser`, `PowerShellParser`
- **`Formatter`** (`src/formatter/mod.rs`) - Shell-specific output formatting
- **`Checker`** (`src/checker/mod.rs`) - Validation rules (duplicates, syntax)

### Command Pattern

Each CLI command lives in `src/cli/commands/<name>.rs` with an `execute()` function.
Commands share state via `CommandContext` from `src/cli/commands/mod.rs`.

### Module Structure

```
src/
├── cli/commands/     # CLI subcommands (list, add, check, etc.)
├── parser/           # Shell parsers
│   ├── bash/         # Bash parser (patterns, control, parsers modules)
│   ├── pwsh/         # PowerShell parser
│   └── builders/     # Multi-line entry builders
├── model/            # Data structures (Entry, EntryType, ShellType)
├── formatter/        # Shell-specific formatters
├── checker/          # Validation checkers
├── backup/           # Backup system
└── utils/            # Shell detection, path utilities
```

## Code Style Guidelines

### Imports

- Group imports: `std` first, then external crates, then `crate::` imports
- Use `use crate::module::Item` for internal imports
- Prefer explicit imports over glob imports (`*`)

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Types/Traits | PascalCase | `EntryType`, `BashParser` |
| Functions/Methods | snake_case | `parse_alias`, `get_shell_type` |
| Constants/Statics | SCREAMING_SNAKE | `ALIAS_SINGLE_RE` |
| Modules | snake_case | `shell_detect`, `code_block` |
| Test functions | `test_<what_it_tests>` | `test_parse_alias_single_quote` |

### Error Handling

- Use `anyhow::Result` for functions that can fail in CLI commands
- Use `thiserror` for library error types requiring pattern matching
- Prefer `?` operator over explicit `match` for error propagation
- Add context with `.context()` or `.with_context()`

### Types and Patterns

- Use builder pattern with `with_*` methods for optional fields
- Implement `Default` for types with sensible defaults
- Derive `Debug, Clone` on data types; add `Serialize, Deserialize` if needed
- Use `Option<T>` for optional fields, not sentinel values

### Documentation

- Add `//!` module docs at top of each file explaining purpose
- Document public traits and their required methods
- Use `///` doc comments for public functions

### Tests

- Place unit tests in `#[cfg(test)] mod tests` at bottom of each file
- Use descriptive test names: `test_<function>_<scenario>`
- Integration tests go in `tests/integration/`
- Use `assert_eq!` with meaningful values, `assert!` for booleans

## Key Implementation Details

### Lenient Parsing

The parser skips unparseable lines with warnings rather than failing. This handles real-world config files with varied syntax.

### Control Structure Awareness

Bash parser tracks `if`/`while`/`for`/`case` depth to only extract top-level definitions, avoiding aliases inside conditional blocks.

### Multi-line Detection

| Entry Type | Start Detection | End Detection |
|------------|-----------------|---------------|
| Function | `func() {` | brace_count = 0 |
| Code Block | `if`/`while`/`for`/`case` | `fi`/`done`/`esac` |
| Alias/Env | Odd single quotes | Even single quotes |

### Regex Patterns

Due to Rust regex limitations (no backreferences), use separate patterns for quote styles:
- `ALIAS_SINGLE_RE` for single quotes
- `ALIAS_DOUBLE_RE` for double quotes

Add new patterns in `src/parser/bash/patterns.rs` using `lazy_static!`.

### Backup System

Backups are created automatically before writes in `~/.config/wenv/backups/<shell>/`.

## Adding New Features

### New Entry Type

1. Add variant to `EntryType` enum in `src/model/entry.rs`
2. Add regex pattern in `src/parser/bash/patterns.rs`
3. Add parse method in `src/parser/bash/parsers.rs`
4. Integrate into main loop in `src/parser/bash/mod.rs`

### New CLI Command

1. Create `src/cli/commands/<name>.rs` with `execute()` function
2. Add command variant to `Commands` enum in `src/cli/args.rs`
3. Add match arm in `src/main.rs`

### New Checker

1. Create module in `src/checker/`
2. Implement `Checker` trait
3. Add to `check_all()` function in `src/checker/mod.rs`

---

## Release Process

发布新版本时，请遵循以下流程：

### Changelog 格式

遵循 [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) 规范，使用以下分类：

| 分类 | 说明 | 使用场景 |
|------|------|----------|
| **Added** | 新增功能 | 全新功能、新的 CLI 命令、新的解析器支持 |
| **Changed** | 现有功能变更 | API 变更、行为修改、默认值调整 |
| **Deprecated** | 即将移除的功能 | 标记将在未来版本移除的功能 |
| **Removed** | 已移除的功能 | 删除的功能、命令或选项 |
| **Fixed** | Bug 修复 | 错误修复、异常处理改进 |
| **Security** | 安全性修复 | 安全漏洞修复、权限问题修正 |

### 发布步骤

#### 1. 整理更新资讯

```bash
# 检视自上次发布以来的所有 commits
git log --oneline <last-tag>..HEAD

# 或查看两个 tag 之间的变更
git log --oneline v0.1.0..HEAD
```

将变更分类至对应的 Changelog 区块。

#### 2. 更新文件

**a) 更新 `CHANGELOG.md`**

将 `[Unreleased]` 区块内容移至新版本区块：

```markdown
## [Unreleased]

## [X.Y.Z] - YYYY-MM-DD

### Added
- Feature A
- Feature B

### Fixed
- Bug fix C
```

**b) 更新 `Cargo.toml`**

```toml
[package]
name = "wenv"
version = "X.Y.Z"  # 更新版本号
```

**c) 更新 `README.md`（如有需要）**

如果新版本包含重大功能变更，更新 README 中的：
- Features 列表
- Usage 示例
- 安装说明

#### 3. 确定版本号

遵循 [Semantic Versioning](https://semver.org/) (SemVer)：

| 版本类型 | 格式 | 使用场景 | 示例 |
|----------|------|----------|------|
| **MAJOR** | X.0.0 | Breaking changes（不兼容的 API 变更） | `1.0.0` → `2.0.0` |
| **MINOR** | 0.X.0 | 新功能（向后兼容） | `0.1.0` → `0.2.0` |
| **PATCH** | 0.0.X | Bug 修复（向后兼容） | `0.1.0` → `0.1.1` |

**决策流程：**
1. 是否有 Breaking changes？ → 增加 MAJOR
2. 是否有新功能（向后兼容）？ → 增加 MINOR
3. 仅有 Bug 修复？ → 增加 PATCH

#### 4. 提交并打 Tag

```bash
# 添加所有变更
git add -A

# 提交变更
git commit -m "chore: release v<VERSION>"

# 创建 annotated tag（推荐）
git tag -a v<VERSION> -m "Release v<VERSION>

<简要 release notes，可包含：>
- Major features / 主要功能
- Breaking changes / 破坏性变更
- Important fixes / 重要修复
"

# 示例
git tag -a v0.2.0 -m "Release v0.2.0

Added:
- PowerShell parser support
- TUI interactive mode
- Backup/restore commands

Fixed:
- Duplicate detection for functions
"
```

**Tag 命名规范：**
- 格式：`vMAJOR.MINOR.PATCH`
- 示例：`v0.1.0`, `v1.2.3`
- 必须包含 `v` 前缀（触发 GitHub Actions）

#### 5. 推送更新

```bash
# 推送 commits
git push origin main

# 推送 tag
git push origin v<VERSION>
```

推送 tag 后，GitHub Actions 将自动：
1. ✅ 构建多平台 binary（Linux, Windows, macOS）
2. ✅ 创建 GitHub Release
3. ✅ 上传构建产物与 SHA256SUMS
4. ✅ 生成自动 Changelog

#### 6. 验证发布

发布完成后，验证以下内容：

- [ ] GitHub Release 页面正常显示
- [ ] 所有平台的 binary 都已上传
- [ ] SHA256SUMS 文件存在且正确
- [ ] Release notes 内容完整
- [ ] 安装脚本能正常下载新版本

**测试安装脚本：**

```powershell
# Windows
$env:APP_NAME="wenv"; $env:REPO="superyngo/wenv"; irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.ps1 | iex
wenv --version
```

```bash
# Linux/macOS
curl -fsSL https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.sh | APP_NAME="wenv" REPO="superyngo/wenv" bash
wenv --version
```

---

### 发布检查清单

在推送 tag 之前，确保：

- [ ] 所有测试通过 (`cargo test`)
- [ ] 代码已格式化 (`cargo fmt`)
- [ ] Clippy 无警告 (`cargo clippy`)
- [ ] CHANGELOG.md 已更新
- [ ] Cargo.toml 版本号已更新
- [ ] README.md 已更新（如有需要）
- [ ] Commit message 使用 `chore: release vX.Y.Z` 格式
- [ ] Tag 使用 annotated tag（`git tag -a`）
- [ ] Tag 包含有意义的 release notes

---

### 发布失败处理

如果 GitHub Actions 构建失败：

1. **检查构建日志**：查看具体错误原因
2. **删除 tag**（本地和远程）：
   ```bash
   git tag -d v<VERSION>
   git push origin :refs/tags/v<VERSION>
   ```
3. **修复问题**：修正代码或配置
4. **重新发布**：从步骤 4 重新开始

---

### 版本发布示例

**场景：发布 v0.2.0，新增 PowerShell 支持**

```bash
# 1. 更新 CHANGELOG.md
# [0.2.0] - 2026-01-15
# ### Added
# - PowerShell parser support

# 2. 更新 Cargo.toml
# version = "0.2.0"

# 3. 提交
git add -A
git commit -m "chore: release v0.2.0"

# 4. 打 tag
git tag -a v0.2.0 -m "Release v0.2.0

Added:
- PowerShell parser support
- PowerShell profile auto-detection
"

# 5. 推送
git push origin main
git push origin v0.2.0

# 6. 等待 GitHub Actions 完成（约 10-15 分钟）
# 7. 验证 https://github.com/superyngo/wenv/releases
```

---

### 常见问题

**Q: 如何发布预发行版本（Pre-release）？**

A: 使用带后缀的版本号：
```bash
git tag -a v0.2.0-beta.1 -m "Beta release"
```

注意：GitHub Actions 会自动将非标准版本号标记为 pre-release。

**Q: 如何更新已发布的 Release notes？**

A: 在 GitHub Release 页面点击 "Edit release" 进行编辑。不建议频繁修改。

**Q: 构建为什么需要这么久？**

A: 因为要构建 14 个不同平台的 binary（包括 musl、ARM 等），预计需要 10-15 分钟。

