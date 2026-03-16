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
├── scanner.rs    # 扫描逻辑 (scan_dir, tree_to_list, get_drives)
├── app.rs        # 应用状态 (App)
├── ui.rs         # UI 渲染 (ratatui 组件)
└── utils.rs      # 工具函数 (format_size)
```

### 核心数据结构 ([models.rs](src/models.rs))

- **`FileNode`** - 表示树中的文件或目录，包含名称、路径、大小和子节点
- **`ScanState`** - 线程安全的扫描状态，使用原子类型实现跨线程通信

### 扫描功能 ([scanner.rs](src/scanner.rs))

- 使用**后台线程**进行目录扫描，保持 UI 响应
- `scan_dir()` 递归遍历目录，跳过隐藏文件（以 `.` 开头）和符号链接
- `tree_to_list()` 将树转换为扁平列表，支持展开/折叠状态
- 结果按大小降序排列（最大的在前面）

### 应用状态 ([app.rs](src/app.rs))

- **`App`** - 主应用程序状态，管理扫描结果、展开的文件夹和 UI 选中状态

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

## 平台支持

- **Windows**: 列出可用驱动器（A-Z）
- **macOS/Linux**: 使用根目录 `/`

应用程序在编译时使用条件编译自动检测平台（`#[cfg(target_os = "windows")]`）