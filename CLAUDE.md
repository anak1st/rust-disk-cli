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

这是一个单文件应用程序 ([src/main.rs](src/main.rs))，包含以下核心组件：

### 核心数据结构

- **`FileNode`** - 表示树中的文件或目录，包含名称、路径、大小和子节点
- **`ScanState`** - 线程安全的扫描状态，使用原子类型实现跨线程通信
- **`App`** - 主应用程序状态，管理扫描结果、展开的文件夹和 UI 选中状态

### 扫描功能

- 使用**后台线程**进行目录扫描，保持 UI 响应
- `scan_dir()` 递归遍历目录，跳过隐藏文件（以 `.` 开头）和符号链接
- 结果按大小降序排列（最大的在前面）

### UI 渲染

- 使用 **ratatui** 构建 TUI 组件
- 使用 **crossterm** 处理终端输入/输出和键盘事件
- 三个主要区域：状态栏（顶部）、文件树（中部）、帮助文字（底部）

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