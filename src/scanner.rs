use crate::models::FileNode;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// 递归扫描目录，构建文件树
///
/// # 参数
/// - path: 要扫描的路径
/// - scanned: 已扫描文件数量的计数器
/// - current_path: 当前扫描路径（用于更新UI）
///
/// # 返回
/// - 成功返回 FileNode，失败返回 io::Error
pub fn scan_dir(
    path: &Path,
    scanned: &Arc<AtomicUsize>,
    current_path: &Arc<std::sync::Mutex<String>>,
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

/// 将树形结构转换为扁平列表，用于在 UI 中显示
///
/// # 参数
/// - node: 当前节点
/// - depth: 当前深度（用于缩进）
/// - expanded: 存储每个路径的展开状态
///
/// # 返回
/// - Vec<(名称, 大小, 深度, 是否目录, 路径)> 元组列表
pub fn tree_to_list(
    node: &FileNode,
    depth: usize,
    expanded: &HashMap<String, bool>,
) -> Vec<(String, String, usize, bool, String)> {
    let mut items = Vec::new();

    // 使用路径作为 key 检查展开状态
    // 根目录默认展开
    let is_expanded = expanded.get(&node.path).copied().unwrap_or(depth == 0);

    // 根据节点类型和展开状态选择前缀图标
    let prefix = if node.is_dir {
        if is_expanded || depth == 0 {
            "📂 " // 已展开的文件夹
        } else {
            "📁 " // 未展开的文件夹
        }
    } else {
        "📄 " // 文件
    };

    // 添加当前节点到列表
    items.push((
        format!("{}{}", prefix, node.name),
        crate::utils::format_size(node.size),
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

/// 获取系统驱动器列表
///
/// # Windows
/// 遍历 A-Z 盘符，返回存在的驱动器
///
/// # macOS / Linux
/// 返回根目录 "/"
#[allow(dead_code)]
pub fn get_drives() -> Vec<String> {
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
