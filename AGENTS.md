# AGENTS.md - 代理编码指南

本文件为在 disk-scanner-cli 代码库中工作的代理提供指导。

## 项目概述

**disk-scanner-cli** 是一个用于分析磁盘空间使用的 Rust TUI 应用程序。它递归扫描目录并显示按大小排序的文件树，帮助用户识别大文件和文件夹。

## 构建/检查/测试命令

```bash
# 构建项目
cargo build

# 发布版本（优化）
cargo build --release

# 运行应用程序
cargo run -- [path]    # 扫描指定路径
cargo run              # 扫描当前目录

# 运行测试
cargo test                    # 运行所有测试
cargo test <test_name>        # 运行特定测试
cargo test -- --nocapture     # 运行测试并显示输出

# 代码检查
cargo clippy                  # 运行 clippy 检查器
cargo clippy --fix            # 自动修复 clippy 建议
cargo clippy -- -D warnings   # 将警告视为错误

# 代码格式化
cargo fmt                    # 格式化代码
cargo fmt -- --check         # 检查格式而不修改

# 完整检查（格式 + clippy + 测试）
cargo fmt -- --check && cargo clippy && cargo test
```

## 代码风格指南

### 格式化
- **缩进**：4 个空格（不使用 Tab）
- **行长度**：软限制 100 个字符
- **不保留尾随空格**
- **使用 `cargo fmt` 自动格式化**

### 导入
- 按外部 crate、内部模块分组导入
- 使用 `use crate::` 引用内部路径
- 使用 `use` 将项目引入作用域

```rust
use crate::models::{FileNode, ListItem};
use rayon::prelude::*;
use std::collections::HashMap;
```

### 命名规范
- **结构体/枚举**：`CamelCase`（例如 `FileNode`、`NodeType`、`ScanState`）
- **函数/方法**：`snake_case`（例如 `scan_dir`、`tree_to_list`、`is_blacklisted`）
- **变量**：`snake_case`（例如 `scan_path`、`file_size`、`is_expanded`）
- **常量**：`SCREAMING_SNAKE_CASE`（例如 `BLACKLIST`、`KB`、`MB`）
- **模块**：`snake_case`（例如 `scanner`、`models`、`utils`）

### 错误处理
- 对可能失败的操作使用 `Result<T, E>`
- 使用 `?` 操作符进行错误早期返回
- 需要时使用 `.map_err(|e| e.to_string())` 转换错误
- 提供有意义的错误消息

```rust
pub fn scan_dir(...) -> Result<FileNode, std::io::Error> {
    let metadata = fs::symlink_metadata(path)?;
    // ...
}
```

### 线程安全
- 使用 `Arc<T>` 实现跨线程的共享所有权
- 使用 `Mutex<T>` 进行内部可变性，同时只能锁定一次
- 使用 `AtomicBool`/`AtomicUsize` 实现简单的原子计数器
- 当不需要严格排序时使用 `Ordering::Relaxed` 以提高性能
- 当需要顺序一致性时使用 `Ordering::SeqCst`（例如信号完成）

```rust
pub struct ScanState {
    pub is_scanning: Arc<AtomicBool>,
    pub current_path: Arc<Mutex<String>>,
    pub files_scanned: Arc<AtomicUsize>,
}
```

### 并发编程
- 使用 `rayon` 进行数据并行操作
- 使用 `into_par_iter()` 进行并行迭代
- 在并行循环中使用 `filter_map` 配合 `.ok()` 来优雅地跳过错误
- 使用 `std::thread::spawn` 生成线程以处理阻塞 I/O

### Rust 惯用法
- 优先使用组合而非继承
- 使用 `impl` 块为结构体定义方法
- 使用 `#[derive(...)]` 为常用 trait 添加派生（`Debug`、`Clone`、`Serialize`、`Deserialize`）
- 使用 `match` 进行穷举处理
- 使用 `if let` / `while let` 进行单一模式匹配
- 使用 `unwrap_or`、`unwrap_or_else`、`unwrap_or_default` 避免 panic
- 使用 `saturating_sub`、`saturating_add` 防止溢出

### 平台特定代码
- 使用 `#[cfg(target_os = "...")]` 处理操作系统特定的路径/功能
- Windows 路径大小写不敏感处理
- macOS：跳过虚拟文件系统（`/dev`、`/Network` 等）
- Linux：跳过虚拟文件系统（`/proc`、`/sys` 等）

### UI 组件 (ratatui)
- 使用 `Frame::render_widget` 绘制小部件
- 使用 `Block::default()` 配合 `.title()`、`.borders()`、`.style()`
- 使用 `Rect::new(x, y, width, height)` 进行定位
- 使用 `Style::default().fg(Color::...).bg(Color::...)` 设置颜色
- 使用 `terminal.size()?.height` 计算可见区域

### 文档
- 为公共 API 添加文档注释（`///`）
- 使用 `# Parameters` 部分记录参数
- 使用 `# Returns` 部分记录返回值
- 必要时包含使用示例

### 性能考虑
- 使用 `symlink_metadata()` 而非 `metadata()` 以避免跟随符号链接
- 跳过符号链接以避免循环引用和重复计数
- 对计数器使用 `fetch_add` 配合 `Ordering::Relaxed`（返回前一个值）
- 定期更新 UI（例如每 1000 个文件）而非每个文件都更新
- 在收集完所有子节点后再排序，而非排序过程中

### 代码组织
```
src/
├── main.rs       # 入口点，事件循环
├── models.rs     # 数据结构（FileNode、ScanState）
├── scanner.rs    # 扫描逻辑（scan_dir、tree_to_list）
├── app.rs        # 应用程序状态（App）
├── ui.rs         # UI 渲染（ratatui 组件）
└── utils.rs      # 工具函数（format_size）
```

### 主要依赖
| Crate | 版本 | 用途 |
|-------|---------|---------|
| ratatui | 0.29 | TUI 框架 |
| crossterm | 0.28 | 终端控制 |
| serde | 1 | 序列化 |
| rayon | 1.8 | 数据并行 |

### 测试
- 目前没有专门的测试文件
- 通过创建带有 `#[cfg(test)]` 的测试模块来测试单个函数
- 使用 `#[test]` 属性标记测试函数
- 使用 `assert!`、`assert_eq!`、`assert_ne!` 进行断言

### Git 规范
- 使用清晰、简洁的提交消息
- 关注"为什么"而非"是什么"
- 不提交 secrets 或凭证
