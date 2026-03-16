# disk-scanner-cli

一个基于 Rust 的终端 UI (TUI) 应用程序，用于分析磁盘空间使用情况。

## 功能特性

- 递归扫描目录并显示按大小排序的文件树
- 多线程并行扫描，利用多核 CPU 提升扫描速度
- 支持展开/折叠目录
- 跨平台支持（Windows、macOS、Linux）
- 自动跳过系统虚拟目录和黑名单路径
- 终端 TUI 界面

## 安装

### 从源码编译

```bash
# 克隆项目
git clone https://github.com/yourusername/disk-scanner-cli.git
cd disk-scanner-cli

# 构建项目
cargo build --release
```

编译后的二进制文件位于 `target/release/disk-scanner-cli`。

## 使用方法

```bash
# 扫描指定路径
cargo run -- /path/to/scan

# 扫描当前目录
cargo run
```

## 键盘控制

| 按键 | 操作 |
|------|------|
| ↑/↓ | 上/下导航 |
| 空格 | 展开/折叠目录 |
| r | 重新扫描当前路径 |
| q | 退出 |

## 项目结构

```
src/
├── main.rs       # 入口点，事件循环
├── models.rs     # 数据结构 (FileNode, ScanState)
├── scanner.rs    # 扫描逻辑
├── app.rs        # 应用状态
├── ui.rs         # UI 渲染
└── utils.rs      # 工具函数
```

## 依赖项

- ratatui 0.29 - TUI 框架
- crossterm 0.28 - 跨平台终端控制
- serde 1 - 序列化支持
- rayon 1.8 - 数据并行库

## 许可证

MIT License
