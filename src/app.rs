use crate::models::{FileNode, ScanState};
use crate::scanner;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::Ordering;

/// 应用程序状态
/// 管理扫描结果、展开状态、用户交互等
pub struct App {
    pub state: ScanState,                                      // 扫描状态
    pub scan_path: String,                                     // 要扫描的路径
    pub expanded: HashMap<String, bool>,                       // 每个路径的展开状态
    pub selected_index: usize,                                 // 当前选中的列表项索引
    pub list_items: Vec<(String, String, usize, bool, String)>, // 扁平化的列表项
    pub scan_thread: Option<std::thread::JoinHandle<Result<FileNode, String>>>, // 扫描线程
    pub scroll_offset: usize,                                  // 列表滚动偏移量
}

impl App {
    /// 创建新的应用实例
    ///
    /// # 参数
    /// - path: 要扫描的目录路径
    pub fn new(path: String) -> Self {
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
            scroll_offset: 0,
        }
    }

    /// 开始扫描目录（非阻塞方式）
    /// 在后台线程中执行扫描，主线程可以继续处理 UI 事件
    pub fn start_scan(&mut self) {
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
        self.state.scan_start_time = Some(std::time::Instant::now());
        self.state.scan_duration_ms = 0;

        // 获取共享的原子引用
        let files_scanned = std::sync::Arc::clone(&self.state.files_scanned);
        let current_path = std::sync::Arc::clone(&self.state.current_path);

        // 启动后台线程执行扫描（不等待完成）
        self.scan_thread = Some(std::thread::spawn(move || {
            scanner::scan_dir(Path::new(&path), &files_scanned, &current_path)
                .map_err(|e| e.to_string())
        }));
    }

    /// 检查扫描是否完成，如果完成则更新结果
    pub fn check_scan_complete(&mut self) {
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
                            // 计算扫描耗时（毫秒）
                            if let Some(start_time) = self.state.scan_start_time {
                                self.state.scan_duration_ms = start_time.elapsed().as_millis();
                            }
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
    pub fn update_list(&mut self) {
        if let Some(ref root) = self.state.root {
            self.list_items = scanner::tree_to_list(root, 0, &self.expanded);
        }
    }

    /// 切换选中项目的展开/折叠状态
    pub fn toggle_expand(&mut self) {
        if self.selected_index < self.list_items.len() {
            let (_, _, depth, is_dir, path) = &self.list_items[self.selected_index];

            // 只有目录才能展开/折叠
            if *is_dir {
                // 使用路径作为 key 切换展开状态
                // 默认值：根目录默认展开，其他默认折叠
                let default = *depth == 0;
                let current = self.expanded.get(path).copied().unwrap_or(default);
                self.expanded.insert(path.clone(), !current);
                self.update_list();
            }
        }
    }

    /// 移动选中项
    ///
    /// # 参数
    /// - delta: 移动的方向和步数（负数向上，正数向下）
    pub fn move_selection(&mut self, delta: isize) {
        if self.list_items.is_empty() {
            return;
        }
        let new_index = (self.selected_index as isize + delta)
            .clamp(0, self.list_items.len() as isize - 1) as usize;
        self.selected_index = new_index;
    }

    /// 更新滚动偏移量，确保选中项在可见区域内
    ///
    /// # 参数
    /// - visible_height: 列表可见区域的高度（行数）
    pub fn update_scroll(&mut self, visible_height: usize) {
        if self.list_items.is_empty() {
            self.scroll_offset = 0;
            return;
        }

        // 如果选中项在滚动区域上方，向上滚动
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }
        // 如果选中项在滚动区域下方，向下滚动
        else if self.selected_index >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected_index.saturating_sub(visible_height - 1);
        }

        // 确保滚动偏移量不超过列表范围
        let max_offset = self.list_items.len().saturating_sub(visible_height);
        self.scroll_offset = self.scroll_offset.min(max_offset);
    }
}
