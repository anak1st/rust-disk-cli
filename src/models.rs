use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// 文件或目录节点
/// 用于表示扫描结果中的每个文件或文件夹
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileNode {
    pub name: String,           // 文件/文件夹名称
    pub path: String,           // 完整路径
    pub is_dir: bool,           // 是否为目录
    pub size: u64,              // 文件大小（字节）
    pub children: Vec<FileNode>, // 子节点列表
}

/// 扫描状态（线程安全版本）
/// 使用原子类型实现线程间共享
pub struct ScanState {
    pub root: Option<FileNode>,              // 扫描结果的根节点
    pub is_scanning: Arc<AtomicBool>,        // 是否正在扫描（原子类型）
    pub current_path: Arc<Mutex<String>>,    // 当前正在扫描的路径
    pub files_scanned: Arc<AtomicUsize>,     // 已扫描的文件数量（原子类型）
    pub error: Option<String>,               // 错误信息（如果有）
    pub scan_start_time: Option<Instant>,    // 扫描开始时间
    pub scan_duration_ms: u128,              // 扫描耗时（毫秒）
}

impl ScanState {
    /// 创建新的扫描状态
    pub fn new() -> Self {
        Self {
            root: None,
            is_scanning: Arc::new(AtomicBool::new(false)),
            current_path: Arc::new(Mutex::new(String::new())),
            files_scanned: Arc::new(AtomicUsize::new(0)),
            error: None,
            scan_start_time: None,
            scan_duration_ms: 0,
        }
    }
}
