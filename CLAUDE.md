# CLAUDE.md

此文件为 Claude Code (claude.ai/code) 在本项目中工作时提供指导。

## 项目概述

**disk-scanner-cli** 是一个基于 Rust 的终端 UI (TUI) 应用程序，用于分析磁盘空间使用情况。它递归扫描目录并显示按大小排序的文件树，帮助用户识别大文件和文件夹。

## 常用命令

```bash
# 构建项目
cargo build

# 优化构建
cargo build --release

# 指定路径运行
cargo run -- /path/to/scan

# 使用当前目录运行（默认）
cargo run
```

编译后的二进制文件位于 `target/debug/disk-scanner-cli`（调试版）或 `target/release/disk-scanner-cli`（发布版）。

## 架构

代码已拆分为多个模块：

```
src/
├── main.rs       # 入口点，事件循环
├── models.rs     # 数据结构 (FileNode, ScanState)
├── scanner.rs    # 扫描逻辑 (scan_dir, tree_to_list, get_drives, is_blacklisted)
├── app.rs        # 应用状态 (App)
├── ui.rs         # UI 渲染 (ratatui 组件)
└── utils.rs      # 工具函数 (format_size)
```

### 核心数据结构 ([models.rs](src/models.rs))

- **`FileNode`** - 表示树中的文件或目录，包含名称、路径、大小和子节点
- **`ScanState`** - 线程安全的扫描状态，使用原子类型实现跨线程通信

### 扫描功能 ([scanner.rs](src/scanner.rs))

#### 多线程并行扫描
- 使用 **rayon** 库实现数据并行，`into_par_iter()` 并行扫描子目录
- 自动利用多核 CPU，显著提升扫描速度
- 线程池由 rayon 自动管理，无需手动配置

#### 性能优化
- **单次系统调用**：使用 `symlink_metadata()` 一次获取文件类型和大小
- **符号链接处理**：跳过符号链接避免循环引用和重复计算
- **黑名单过滤**：跳过系统虚拟目录，避免无意义的扫描

#### 平台黑名单

**macOS:**
- 虚拟文件系统：`/dev`, `/.vol`, `/Network`
- 系统数据卷：`/System/Volumes`
- 系统内部数据：`/var/db`, `/private/var/db`, `/private/var/vm`
- 文件系统元数据：`/.Spotlight-V100`, `/.Trashes`, `/.fseventsd`

**Linux:**
- 虚拟文件系统：`/proc`, `/sys`, `/dev`, `/run`
- 系统目录：`/boot`, `/lost+found`, `/snap`

**Windows:**
- 回收站：`\$recycle.bin`
- 系统还原：`\system volume information`
- 安装缓存：`\config.msi`
- 系统日志：`\intel`, `\perflogs`

#### 其他功能
- `tree_to_list()` 将树转换为扁平列表，支持展开/折叠状态
- 结果按大小降序排列（最大的在前面）
- 跳过隐藏文件（以 `.` 开头）

### 应用状态 ([app.rs](src/app.rs))

- **`App`** - 主应用程序状态，管理扫描结果、展开的文件夹和 UI 选中状态
- 使用后台线程进行目录扫描，保持 UI 响应
- `check_scan_complete()` 非阻塞检查扫描状态

### UI 渲染 ([ui.rs](src/ui.rs))

- 使用 **ratatui** 构建 TUI 组件
- 使用 **crossterm** 处理终端输入/输出和键盘事件
- 三个主要区域：状态栏（顶部）、文件树（中部）、帮助文字（底部）
- 文件名根据缩进深度动态调整宽度，确保数字列对齐
- 图标：📂 已展开文件夹，📁 未展开文件夹，📄 文件

### 键盘控制

| 按键 | 操作 |
|------|------|
| ↑/↓ | 上/下导航 |
| 空格 | 展开/折叠目录 |
| r | 重新扫描当前路径 |
| q | 退出 |

## 依赖项

| 依赖 | 版本 | 用途 |
|------|------|------|
| ratatui | 0.29 | TUI 框架 |
| crossterm | 0.28 | 跨平台终端控制 |
| serde | 1 | 序列化支持 |
| rayon | 1.8 | 数据并行库 |

## 平台支持

- **Windows**: 列出可用驱动器（A-Z）
- **macOS/Linux**: 使用根目录 `/`

应用程序在编译时使用条件编译自动检测平台（`#[cfg(target_os = "windows")]`）
