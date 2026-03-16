// 引入 crossterm 库，用于终端输入输出和键盘事件处理
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};

// 引入 ratatui 库，用于构建 TUI（终端用户界面）
use ratatui::{
    backend::CrosstermBackend,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};

// 引入 serde 库，用于 JSON 序列化/反序列化
use serde::{Deserialize, Serialize};

// 引入 HashMap，用于存储每个节点的展开状态
use std::collections::HashMap;

// 引入 env，用于获取命令行参数
use std::env;

// 引入 fs，用于文件系统操作
use std::fs;

// 引入 Path，用于路径处理
use std::path::Path;

// 引入原子操作和线程，用于后台扫描
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// ============================================================================
// 数据结构定义
// ============================================================================

/// 文件或目录节点
/// 用于表示扫描结果中的每个文件或文件夹
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileNode {
    pub name: String,      // 文件/文件夹名称
    pub path: String,      // 完整路径
    pub is_dir: bool,      // 是否为目录
    pub size: u64,         // 文件大小（字节）
    pub children: Vec<FileNode>,  // 子节点列表
}

/// 扫描状态（线程安全版本）
/// 使用原子类型实现线程间共享
struct ScanState {
    root: Option<FileNode>,           // 扫描结果的根节点
    is_scanning: Arc<AtomicBool>,    // 是否正在扫描（原子类型）
    current_path: Arc<Mutex<String>>, // 当前正在扫描的路径
    files_scanned: Arc<AtomicUsize>, // 已扫描的文件数量（原子类型）
    error: Option<String>,           // 错误信息（如果有）
}

impl ScanState {
    /// 创建新的扫描状态
    fn new() -> Self {
        Self {
            root: None,
            is_scanning: Arc::new(AtomicBool::new(false)),
            current_path: Arc::new(Mutex::new(String::new())),
            files_scanned: Arc::new(AtomicUsize::new(0)),
            error: None,
        }
    }
}

// ============================================================================
// 工具函数
// ============================================================================

/// 格式化文件大小，将字节转换为人类可读的格式
/// 例如: 1024 -> "1.00 KB", 1024*1024 -> "1.00 MB"
fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if size >= TB {
        format!("{:.2} TB", size as f64 / TB as f64)
    } else if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} B", size)
    }
}

// ============================================================================
// 核心扫描功能
// ============================================================================

/// 递归扫描目录，构建文件树
///
/// # 参数
/// - path: 要扫描的路径
/// - scanned: 已扫描文件数量的计数器
///
/// # 返回
/// - 成功返回 FileNode，失败返回 io::Error
fn scan_dir(
    path: &Path,
    scanned: &Arc<AtomicUsize>,
    current_path: &Arc<Mutex<String>>
) -> Result<FileNode, std::io::Error> {
    // 增加已扫描文件计数
    scanned.fetch_add(1, Ordering::Relaxed);

    // 每扫描 100 个文件更新一次当前路径显示
    if scanned.load(Ordering::Relaxed) % 100 == 0 {
        if let Ok(mut cp) = current_path.lock() {
            *cp = path.to_string_lossy().to_string();
        }
    }

    // 获取文件/文件夹名称
    // 如果没有名称（如根目录），则使用完整路径
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());

    // 使用 symlink_metadata 检查是否是符号链接
    // 注意：这里不能使用 fs::metadata，因为它会跟随符号链接
    let symlink_meta = fs::symlink_metadata(path)?;

    // 如果是符号链接，跳过不扫描（避免循环引用和重复计算）
    if symlink_meta.file_type().is_symlink() {
        return Ok(FileNode {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir: false,
            size: 0,
            children: vec![],
        });
    }

    // 获取实际的文件元数据
    let metadata = fs::metadata(path)?;

    // 如果是普通文件，返回文件节点
    if metadata.is_file() {
        return Ok(FileNode {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir: false,
            size: metadata.len(),
            children: vec![],
        });
    }

    // 如果是目录，递归扫描所有子项
    let mut children = Vec::new();
    let mut total_size: u64 = 0;

    // 尝试读取目录内容
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();

            // 跳过隐藏文件（以 . 开头的文件）
            let file_name = entry.file_name();
            if file_name.to_string_lossy().starts_with('.') {
                continue;
            }

            // 递归扫描子项，忽略错误（如权限不足的目录）
            match scan_dir(&entry_path, scanned, current_path) {
                Ok(child) => {
                    total_size += child.size;
                    children.push(child);
                }
                Err(_) => {
                    // 跳过无法访问的文件/目录
                }
            }
        }
    }

    // 按大小降序排序子项，最大的文件/文件夹显示在前面
    children.sort_by(|a, b| b.size.cmp(&a.size));

    Ok(FileNode {
        name,
        path: path.to_string_lossy().to_string(),
        is_dir: true,
        size: total_size,
        children,
    })
}

/// 获取系统驱动器列表
///
/// # Windows
/// 遍历 A-Z 盘符，返回存在的驱动器
///
/// # macOS / Linux
/// 返回根目录 "/"
#[allow(dead_code)]
fn get_drives() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        let mut drives = Vec::new();
        // 遍历 A-Z 检查每个驱动器是否存在
        for letter in b'A'..=b'Z' {
            let drive = format!("{}:", letter as char);
            if Path::new(&drive).exists() {
                drives.push(drive);
            }
        }
        drives
    }
    #[cfg(not(target_os = "windows"))]
    {
        // 非 Windows 系统返回根目录
        vec!["/".to_string()]
    }
}

// ============================================================================
// 树形结构转换
// ============================================================================

/// 将树形结构转换为扁平列表，用于在 UI 中显示
///
/// # 参数
/// - node: 当前节点
/// - depth: 当前深度（用于缩进）
/// - expanded: 存储每个路径的展开状态
///
/// # 返回
/// - Vec<(名称, 大小, 深度, 是否目录, 路径)> 元组列表
fn tree_to_list(
    node: &FileNode,
    depth: usize,
    expanded: &HashMap<String, bool>
) -> Vec<(String, String, usize, bool, String)> {
    let mut items = Vec::new();

    // 使用路径作为 key 检查展开状态
    // 根目录默认展开
    let is_expanded = expanded.get(&node.path).copied().unwrap_or(depth == 0);

    // 根据展开状态选择前缀符号
    let prefix = if depth == 0 {
        "📁 "  // 根目录
    } else if is_expanded {
        "▼ "   // 已展开
    } else {
        "▶ "   // 已折叠
    };

    // 添加当前节点到列表
    items.push((
        format!("{}{}", prefix, node.name),
        format_size(node.size),
        depth,
        node.is_dir,
        node.path.clone(),
    ));

    // 如果是目录且已展开，递归添加子节点
    if node.is_dir && is_expanded {
        for child in &node.children {
            items.extend(tree_to_list(child, depth + 1, expanded));
        }
    }

    items
}

// ============================================================================
// 应用状态管理
// ============================================================================

/// 应用程序状态
/// 管理扫描结果、展开状态、用户交互等
struct App {
    state: ScanState,                                      // 扫描状态
    scan_path: String,                                     // 要扫描的路径
    expanded: HashMap<String, bool>,                       // 每个路径的展开状态
    selected_index: usize,                                 // 当前选中的列表项索引
    list_items: Vec<(String, String, usize, bool, String)>, // 扁平化的列表项
    scan_thread: Option<std::thread::JoinHandle<Result<FileNode, String>>>, // 扫描线程
}

impl App {
    /// 创建新的应用实例
    ///
    /// # 参数
    /// - path: 要扫描的目录路径
    fn new(path: String) -> Self {
        let mut expanded = HashMap::new();
        // 根目录默认展开
        expanded.insert(path.clone(), true);
        Self {
            state: ScanState::new(),
            scan_path: path,
            expanded,
            selected_index: 0,
            list_items: Vec::new(),
            scan_thread: None,
        }
    }

    /// 开始扫描目录（非阻塞方式）
    /// 在后台线程中执行扫描，主线程可以继续处理 UI 事件
    fn start_scan(&mut self) {
        let path = self.scan_path.clone();

        // 如果有正在运行的扫描线程，先等待它结束
        if let Some(handle) = self.scan_thread.take() {
            let _ = handle.join();
        }

        // 重置状态
        self.state.root = None;
        self.state.error = None;
        self.state.is_scanning.store(true, Ordering::SeqCst);
        self.state.files_scanned.store(0, Ordering::SeqCst);

        // 获取共享的原子引用
        let files_scanned = Arc::clone(&self.state.files_scanned);
        let current_path = Arc::clone(&self.state.current_path);

        // 启动后台线程执行扫描（不等待完成）
        self.scan_thread = Some(std::thread::spawn(move || {
            scan_dir(Path::new(&path), &files_scanned, &current_path)
                .map_err(|e| e.to_string())
        }));
    }

    /// 检查扫描是否完成，如果完成则更新结果
    fn check_scan_complete(&mut self) {
        // 1. 先检查是否有线程句柄
        if let Some(handle) = self.scan_thread.as_ref() {
            // 2. 使用 is_finished() 进行非阻塞检查
            if handle.is_finished() {
                // 3. 线程已完成，取出句柄并获取结果
                if let Some(handle) = self.scan_thread.take() {
                    match handle.join() {
                        Ok(result) => {
                            // 扫描彻底结束，设置原子变量
                            self.state.is_scanning.store(false, Ordering::SeqCst);
                            match result {
                                Ok(root) => {
                                    self.state.root = Some(root);
                                    self.update_list();
                                }
                                Err(e) => {
                                    self.state.error = Some(e);
                                }
                            }
                        }
                        Err(_) => {
                            self.state.is_scanning.store(false, Ordering::SeqCst);
                            self.state.error = Some("扫描线程崩溃 (Panicked)".to_string());
                        }
                    }
                }
            }
        }
    }

    /// 更新扁平化的列表
    /// 根据当前的展开状态重新生成列表
    fn update_list(&mut self) {
        if let Some(ref root) = self.state.root {
            self.list_items = tree_to_list(root, 0, &self.expanded);
        }
    }

    /// 切换选中项目的展开/折叠状态
    fn toggle_expand(&mut self) {
        if self.selected_index < self.list_items.len() {
            let (_, _, depth, is_dir, path) = &self.list_items[self.selected_index];

            // 只有目录才能展开/折叠
            if *is_dir {
                // 使用路径作为 key 切换展开状态
                // 默认值需要和 tree_to_list 保持一致：depth == 0 则默认展开
                let current = self.expanded.get(path).copied().unwrap_or(*depth == 0);
                self.expanded.insert(path.clone(), !current);
                self.update_list();
            }
        }
    }

    /// 移动选中项
    ///
    /// # 参数
    /// - delta: 移动的方向和步数（负数向上，正数向下）
    fn move_selection(&mut self, delta: isize) {
        let new_index = (self.selected_index as isize + delta)
            .clamp(0, self.list_items.len() as isize - 1) as usize;
        self.selected_index = new_index;
    }
}

// ============================================================================
// 主程序入口
// ============================================================================

/// 程序入口点
///
/// 使用方法:
/// - disk-scanner-cli [路径]  - 扫描指定路径
/// - disk-scanner-cli          - 扫描当前目录
///
/// 键盘控制:
/// - ↑/↓: 上下移动选择
/// - 空格: 展开/折叠目录
/// - r: 重新扫描
/// - q: 退出程序
fn main() -> std::io::Result<()> {
    // -----------------------------------------------------------------------
    // 1. 解析命令行参数
    // -----------------------------------------------------------------------
    let args: Vec<String> = env::args().collect();

    // 如果提供了参数，使用第一个参数作为扫描路径；否则使用当前目录
    let scan_path = if args.len() > 1 {
        args[1].clone()
    } else {
        // 默认当前目录
        ".".to_string()
    };

    // 验证路径是否存在
    if !Path::new(&scan_path).exists() {
        eprintln!("错误: 路径不存在: {}", scan_path);
        eprintln!("用法: disk-scanner-cli [路径]");
        std::process::exit(1);
    }

    // -----------------------------------------------------------------------
    // 2. 初始化终端界面
    // -----------------------------------------------------------------------
    // 进入备用屏幕缓冲区
    std::io::stdout().execute(EnterAlternateScreen)?;

    // 启用原始模式（禁用行缓冲和回显）
    enable_raw_mode()?;

    // 创建 ratatui 终端
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    // -----------------------------------------------------------------------
    // 3. 创建应用并开始扫描
    // -----------------------------------------------------------------------
    let mut app = App::new(scan_path);

    // 自动开始扫描
    app.start_scan();

    // -----------------------------------------------------------------------
    // 4. 主事件循环
    // -----------------------------------------------------------------------
    loop {
        // 检查扫描是否完成
        app.check_scan_complete();

        // 渲染界面
        terminal.draw(|f| {
            let size = f.area();

            // -------------------------------------------------------------------
            // 4.1 绘制顶部状态栏
            // -------------------------------------------------------------------
            let status_bar = Block::default()
                .title("Disk Scanner - 磁盘空间分析工具")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::DarkGray));

            f.render_widget(status_bar, Rect::new(0, 0, size.width, 3));

            // 根据扫描状态显示不同信息
            let status_text = if app.state.is_scanning.load(Ordering::Relaxed) {
                let files = app.state.files_scanned.load(Ordering::Relaxed);
                let path = app.state.current_path.lock().map(|p| p.clone()).unwrap_or_default();
                format!("扫描中: {} ({} 文件)", path, files)
            } else if let Some(ref root) = app.state.root {
                format!("总大小: {}", format_size(root.size))
            } else {
                "未扫描".to_string()
            };

            let status_widget = Paragraph::new(status_text)
                .style(Style::default().fg(Color::White));
            f.render_widget(status_widget, Rect::new(1, 1, size.width - 2, 1));

            // -------------------------------------------------------------------
            // 4.2 绘制文件列表
            // -------------------------------------------------------------------
            let list_items: Vec<ListItem> = app
                .list_items
                .iter()
                .enumerate()
                .map(|(i, (name, size, depth, _, _))| {
                    // 计算缩进（每个深度级别 2 个空格）
                    let indent = "  ".repeat(*depth);

                    // 格式化显示内容：名称 + 大小
                    let content = format!("{}{:>12}", name, size);

                    // 高亮当前选中的项
                    let style = if i == app.selected_index {
                        Style::default().bg(Color::Blue).fg(Color::White)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    ListItem::new(format!("{}{}", indent, content)).style(style)
                })
                .collect();

            let list = List::new(list_items)
                .block(Block::default().title("文件树").borders(Borders::ALL))
                .style(Style::default().bg(Color::Black));

            f.render_widget(
                list,
                Rect::new(0, 3, size.width, size.height.saturating_sub(4)),
            );

            // -------------------------------------------------------------------
            // 4.3 绘制底部帮助信息
            // -------------------------------------------------------------------
            let help_text = Paragraph::new("↑↓ 选择 | 空格 展开/折叠 | r 重新扫描 | q 退出")
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(
                help_text,
                Rect::new(0, size.height.saturating_sub(1), size.width, 1),
            );

            // -------------------------------------------------------------------
            // 4.4 绘制错误信息
            // -------------------------------------------------------------------
            if let Some(ref error) = app.state.error {
                let error_block = Paragraph::new(format!("错误: {}", error))
                    .style(Style::default().fg(Color::Red).bg(Color::White));
                f.render_widget(
                    error_block,
                    Rect::new(0, size.height.saturating_sub(2), size.width, 1),
                );
            }
        })?;

        // -------------------------------------------------------------------
        // 5. 处理键盘事件
        // -------------------------------------------------------------------
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        // q: 退出程序
                        KeyCode::Char('q') => break,

                        // r: 重新扫描
                        KeyCode::Char('r') => {
                            // 重置展开状态
                            app.expanded.clear();
                            app.expanded.insert(app.scan_path.clone(), true);
                            app.start_scan();
                        }

                        // 空格: 切换展开/折叠
                        KeyCode::Char(' ') => app.toggle_expand(),

                        // ↑: 上移
                        KeyCode::Up => app.move_selection(-1),

                        // ↓: 下移
                        KeyCode::Down => app.move_selection(1),

                        // 其他按键忽略
                        _ => {}
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // 6. 清理退出
    // -----------------------------------------------------------------------
    // 恢复终端状态
    disable_raw_mode()?;
    std::io::stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}
