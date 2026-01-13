# wenv

A cross-platform CLI tool for managing shell configuration files.  
跨平台 Shell 配置文件管理工具。

---

## Features / 功能特点

- ✅ **Cross-platform support** / **跨平台支持** (Linux, Windows, macOS)
- ✅ **Multi-shell parser** / **多 Shell 解析器** (Bash, PowerShell)
- ✅ **Entry management** / **条目管理** (aliases, functions, environment variables, source statements)
- ✅ **Duplicate detection** / **重复检查**
- ✅ **Syntax validation** / **语法验证**
- ✅ **Automatic backups** / **自动备份**
- ✅ **TUI interface** / **终端用户界面**
- ✅ **Multi-language support** / **多语言支持** (English, 繁體中文)

---

## Installation / 安装

### Quick Install (One-Line Command) / 快速安装

#### Windows (PowerShell)

```powershell
$env:APP_NAME="wenv"; $env:REPO="superyngo/wenv"; irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.ps1 | iex
```

**Uninstall / 卸载:**
```powershell
$env:APP_NAME="wenv"; $env:REPO="superyngo/wenv"; $env:UNINSTALL="true"; irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.ps1 | iex
```

#### Linux / macOS (Bash)

```bash
curl -fsSL https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.sh | APP_NAME="wenv" REPO="superyngo/wenv" bash
```

**Uninstall / 卸载:**
```bash
curl -fsSL https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.sh | APP_NAME="wenv" REPO="superyngo/wenv" bash -s uninstall
```

The installation script will: / 安装脚本将：
- Automatically detect your OS and architecture / 自动检测操作系统和架构
- Download the latest precompiled binary from GitHub Releases / 从 GitHub Releases 下载最新预编译二进制文件
- Install to: / 安装到：
  - Windows: `%LOCALAPPDATA%\Programs\wenv`
  - Linux/macOS: `~/.local/bin`
- Add the installation directory to your PATH (if needed) / 将安装目录添加到 PATH（如有需要）

**Supported Platforms / 支持的平台:**
- Windows (x86_64, i686)
- Linux (x86_64, i686, aarch64, armv7) - both GNU and musl
- macOS (x86_64, Apple Silicon)

---

### Manual Installation / 手动安装

#### From Precompiled Binaries / 使用预编译二进制文件

Download the latest release for your platform from the [Releases](https://github.com/superyngo/wenv/releases) page.  
从 [Releases](https://github.com/superyngo/wenv/releases) 页面下载适合您平台的最新版本。

**Windows:**
```powershell
# Extract the downloaded file and move wenv.exe to a directory in your PATH
# 解压下载的文件并将 wenv.exe 移动到 PATH 中的目录
move wenv.exe %LOCALAPPDATA%\Programs\wenv\
```

**Linux/macOS:**
```bash
# Extract the downloaded tar.gz file and move wenv to a directory in your PATH
# 解压下载的 tar.gz 文件并将 wenv 移动到 PATH 中的目录
tar -xzf wenv-*.tar.gz
chmod +x wenv
mv wenv ~/.local/bin/
```

---

#### From Source / 从源代码编译

If you prefer to build from source, ensure you have [Rust](https://rustup.rs/) installed:  
如果您希望从源代码编译，请确保已安装 [Rust](https://rustup.rs/)：

```bash
# Clone the repository / 克隆仓库
git clone https://github.com/superyngo/wenv.git
cd wenv

# Build release binary / 构建 Release 版本
cargo build --release

# The binary will be available at: / 二进制文件将位于：
# - Windows: target\release\wenv.exe
# - Linux/macOS: target/release/wenv

# Install manually / 手动安装
# Windows:
copy target\release\wenv.exe %LOCALAPPDATA%\Programs\wenv\

# Linux/macOS:
cp target/release/wenv ~/.local/bin/
chmod +x ~/.local/bin/wenv
```

---

## Usage / 使用方式

### Basic Usage / 基本使用

```bash
# Launch TUI (default) / 启动 TUI 交互界面（默认）
wenv

# Specify shell configuration file / 指定 shell 配置文件
wenv --file ~/.bashrc          # Bash
wenv --file $PROFILE           # PowerShell

# Specify shell type explicitly / 明确指定 shell 类型
wenv --shell bash --file ~/.custom_rc
wenv --shell pwsh --file custom_profile.ps1
```

### TUI Interface / TUI 交互界面

The default TUI interface provides:  
默认的 TUI 界面提供：

- **Browse** / **浏览**: View all parsed entries (aliases, functions, env vars, source statements)
- **Search** / **搜索**: Filter entries by name or type
- **Edit** / **编辑**: Modify entries directly
- **Add** / **添加**: Create new entries
- **Delete** / **删除**: Remove unwanted entries
- **Copy/Paste** / **复制/粘贴**: Copy entries with Ctrl+C and paste with Ctrl+V
- **Format** / **格式化**: Auto-format with preview and confirmation
- **Save** / **保存**: Apply changes to configuration file (with automatic backup)

### Format Operation / 格式化操作

The format function (press `f` in TUI) performs the following operations with a preview before applying:  
格式化功能（TUI 中按 `f`）在应用前提供预览，执行以下操作：

| Operation / 操作 | Description / 说明 |
|------------------|-------------------|
| **Preview changes** / **预览变更** | Shows a summary of all changes before applying / 在应用前显示所有变更摘要 |
| **Duplicate detection** / **重复检测** | Warns about duplicate entries / 警告重复条目 |
| **PATH merging** / **PATH 合并** | Merges multiple PATH definitions into one / 将多个 PATH 定义合并为一个 |
| **Group by type** / **按类型分组** | Groups entries at their first occurrence position / 在首次出现位置对条目分组 |
| **Alphabetical sort** / **字母排序** | Sorts entries within groups / 组内条目按字母顺序排序 |
| **Dependency ordering** / **依赖排序** | Sorts environment variables by dependency (topological sort) / 环境变量按依赖关系排序 |

**Usage / 使用方法:**
1. Press `f` in TUI to start format / 在 TUI 中按 `f` 开始格式化
2. Review the preview showing all changes / 查看显示所有变更的预览
3. Press `y` to apply or `n` to cancel / 按 `y` 应用或 `n` 取消

### Quick Actions / 快速操作

For non-interactive operations, use these flags:  
对于非交互操作，使用以下参数：

```bash
# Import entries from file or URL / 从文件或 URL 导入条目
wenv --import /path/to/aliases.sh
wenv --import https://example.com/my-aliases.sh

# Import with conflict handling / 导入时处理冲突
wenv --import aliases.sh --on-conflict skip      # Skip duplicates / 跳过重复项
wenv --import aliases.sh --on-conflict overwrite # Overwrite existing / 覆盖现有项
wenv --import aliases.sh --yes                   # Skip confirmation / 跳过确认

# Export entries to file / 导出条目到文件
wenv --export my-backup.sh

# Export specific entry types / 导出特定类型的条目
wenv --export aliases-only.sh --type alias
wenv --export functions.sh --type func

# Open source file in $EDITOR / 在 $EDITOR 中打开源文件
wenv --source
wenv --file ~/.bashrc --source

# Show help / 显示帮助
wenv --help

# Show version / 显示版本
wenv --version
```

---

## Command-Line Options / 命令行选项

| Option / 选项 | Description / 说明 |
|---------------|-------------------|
| (no args) | Launch TUI interface / 启动 TUI 交互界面 |
| `-f, --file <FILE>` | Specify configuration file path / 指定配置文件路径 |
| `-S, --shell <SHELL>` | Specify shell type (bash, pwsh) / 指定 shell 类型 |
| `-i, --import <SOURCE>` | Import entries from file or URL / 从文件或 URL 导入条目 |
| `-e, --export <OUTPUT>` | Export entries to file / 导出条目到文件 |
| `-s, --source` | Open source file in $EDITOR / 在 $EDITOR 中打开源文件 |
| `-t, --type <TYPE>` | Filter by entry type (for export) / 按条目类型过滤（用于导出） |
| `--on-conflict <STRATEGY>` | Conflict handling (ask/skip/overwrite) / 冲突处理策略 |
| `-y, --yes` | Skip confirmation prompts / 跳过确认提示 |
| `-h, --help` | Print help / 显示帮助 |
| `-V, --version` | Print version / 显示版本 |

**Entry Types / 条目类型:**
- `alias` - Command alias / 命令别名
- `func` - Shell function / Shell 函数
- `env` - Environment variable / 环境变量
- `source` - Source statement / Source 语句
- `code` - Code block / 代码块
- `comment` - Comment / 注释

---

## Shell Support / Shell 支持

> **Note**: wenv currently supports **Bash** and **PowerShell** only. Other shells (zsh, fish, etc.) are not yet supported.
>
> **注意**: wenv 目前**仅支援 Bash 和 PowerShell**。其他 shell（zsh、fish 等）尚未支援。

### Bash

Supported configuration files / 支持的配置文件:
- `~/.bashrc`
- `~/.bash_profile`
- `~/.profile`

Supported entry types / 支持的条目类型:
- Aliases: `alias name='value'`
- Functions: `function_name() { ... }`
- Environment variables: `export VAR=value`
- Source statements: `source /path/to/file`

### PowerShell

Supported configuration files / 支持的配置文件:
- `$PROFILE` (CurrentUserCurrentHost)
- `$PROFILE.CurrentUserAllHosts`
- `$PROFILE.AllUsersCurrentHost`
- `$PROFILE.AllUsersAllHosts`

Supported entry types / 支持的条目类型:
- Aliases: `Set-Alias name value`
- Functions: `function Name { ... }`
- Environment variables: `$env:VAR = "value"`
- Source statements: `. /path/to/file.ps1`

---

## Development / 开发

### Build / 构建

```bash
cargo build              # Debug build / 调试构建
cargo build --release    # Release build / 发布构建
cargo check              # Fast syntax check / 快速语法检查
```

### Test / 测试

```bash
cargo test               # Run all tests / 运行所有测试
cargo test --lib         # Library tests only / 仅库测试
cargo test --test integration  # Integration tests / 集成测试
```

### Lint and Format / 代码检查和格式化

```bash
cargo fmt                # Format code / 格式化代码
cargo clippy             # Lint with clippy / Clippy 检查
cargo fmt -- --check     # Check formatting / 检查格式
```

### Run / 运行

```bash
cargo run -- list                    # Run with subcommand / 运行子命令
cargo run -- --file ~/.bashrc list   # Specify config file / 指定配置文件
cargo run -- --help                  # Show help / 显示帮助
```

For more details, see [AGENTS.md](AGENTS.md).  
更多详情请参见 [AGENTS.md](AGENTS.md)。

---

## Architecture / 架构

wenv follows a trait-based design for extensibility:  
wenv 采用基于 trait 的设计以实现可扩展性：

- **Parser** - Shell-specific parsers (Bash, PowerShell) / Shell 特定解析器
- **Formatter** - Shell-specific output formatting / Shell 特定输出格式化
- **Checker** - Validation rules (duplicates, syntax) / 验证规则（重复、语法）

For architecture details, see [AGENTS.md](AGENTS.md).  
架构详情请参见 [AGENTS.md](AGENTS.md)。

---

## Backup System / 备份系统

wenv automatically creates backups before modifying configuration files:  
wenv 在修改配置文件前自动创建备份：

- **Backup location / 备份位置:** `~/.config/wenv/backups/<shell>/`
- **Naming format / 命名格式:** `<original_filename>.<timestamp>.bak`
- **Auto-backup / 自动备份:** Triggered whenever you save changes in TUI mode / 在 TUI 模式中保存变更时自动触发

Backups are managed automatically - no manual commands needed.  
备份自动管理 - 无需手动命令。

---

## License / 许可证

MIT License

---

## Contributing / 贡献

Contributions are welcome! Please feel free to submit a Pull Request.  
欢迎贡献！请随时提交 Pull Request。

---

## Acknowledgments / 致谢

Built with ❤️ using:
- [clap](https://github.com/clap-rs/clap) - CLI argument parsing / CLI 参数解析
- [ratatui](https://github.com/ratatui-org/ratatui) - TUI framework / TUI 框架
- [regex](https://github.com/rust-lang/regex) - Regular expressions / 正则表达式
- [anyhow](https://github.com/dtolnay/anyhow) - Error handling / 错误处理

---

**For detailed documentation, see [AGENTS.md](AGENTS.md).**  
**详细文档请参见 [AGENTS.md](AGENTS.md)。**
