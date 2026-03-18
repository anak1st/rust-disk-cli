use crate::models::{FileNode, NodeType};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// 检查路径是否在黑名单中
/// 根据操作系统返回不同的黑名单
fn is_blacklisted(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    #[cfg(target_os = "macos")]
    {
        // 只排除虚拟文件系统和系统内部目录
        // 正常的系统文件如 /System/Library 等仍然可以扫描
        const BLACKLIST: &[&str] = &[
            // 虚拟文件系统
            "/dev",
            "/.vol",
            "/Network",
            // 系统数据卷（包含实际用户数据，但路径特殊）
            "/System/Volumes",
            // 系统内部数据库和缓存
            "/var/db",
            "/var/log/asl",
            "/private/var/db",
            "/private/var/vm",
            // 文件系统元数据
            "/.Spotlight-V100",
            "/.Trashes",
            "/.fseventsd",
            // 外部卷根目录（可选，如果只想扫描本地磁盘）
            // "/Volumes",
        ];
        BLACKLIST.iter().any(|&p| path_str == p || path_str.starts_with(&format!("{}/", p)))
    }

    #[cfg(target_os = "linux")]
    {
        const BLACKLIST: &[&str] = &[
            "/proc",
            "/sys",
            "/dev",
            "/run",
            "/boot",
            "/lost+found",
            "/snap",
        ];
        BLACKLIST.iter().any(|&p| path_str == p || path_str.starts_with(&format!("{}/", p)))
    }

    #[cfg(target_os = "windows")]
    {
        // Windows 路径不区分大小写
        let path_lower = path_str.to_lowercase();
        const BLACKLIST: &[&str] = &[
            r"\$recycle.bin",
            r"\system volume information",
            r"\config.msi",
            r"\intel",
            r"\perflogs",
        ];
        BLACKLIST.iter().any(|&p| {
            let p_lower = p.to_lowercase();
            path_lower.contains(&p_lower)
        })
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        false
    }
}

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
    // 增加已扫描文件计数，并获取增加前的值
    // 合并为一个原子操作，避免两次原子操作之间的竞争
    let prev_count = scanned.fetch_add(1, Ordering::Relaxed);

    // 每扫描 1000 个文件更新一次当前路径显示
    if prev_count % 1000 == 0 {
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

    // 检查路径是否在黑名单中
    if is_blacklisted(path) {
        return Ok(FileNode {
            name,
            path: path.to_string_lossy().to_string(),
            node_type: NodeType::Skipped,
            size: 0,
            children: vec![],
        });
    }

    // 使用 symlink_metadata 获取元数据（不跟随符号链接）
    // 一次系统调用获取所有需要的信息
    let metadata = fs::symlink_metadata(path)?;
    let file_type = metadata.file_type();

    // 如果是符号链接，跳过不扫描（避免循环引用和重复计算）
    if file_type.is_symlink() {
        return Ok(FileNode {
            name,
            path: path.to_string_lossy().to_string(),
            node_type: NodeType::Symlink,
            size: 0,
            children: vec![],
        });
    }

    // 如果是普通文件，返回文件节点
    if file_type.is_file() {
        return Ok(FileNode {
            name,
            path: path.to_string_lossy().to_string(),
            node_type: NodeType::File,
            size: metadata.len(),
            children: vec![],
        });
    }

    // 如果是目录，并行递归扫描所有子项
    let mut children = Vec::new();

    // 尝试读取目录内容
    if let Ok(entries) = fs::read_dir(path) {
        // 收集所有条目（不过滤隐藏文件）
        let entries: Vec<_> = entries
            .flatten()
            .collect();

        // 使用 rayon 并行扫描子目录
        children = entries
            .into_par_iter()
            .filter_map(|entry| {
                let entry_path = entry.path();
                // 递归扫描子项，忽略错误（如权限不足的目录）
                scan_dir(&entry_path, scanned, current_path).ok()
            })
            .collect();
    }

    // 计算总大小
    let total_size: u64 = children.iter().map(|c| c.size).sum();

    // 按大小降序排序子项，最大的文件/文件夹显示在前面
    children.sort_by(|a, b| b.size.cmp(&a.size));

    Ok(FileNode {
        name,
        path: path.to_string_lossy().to_string(),
        node_type: NodeType::Directory,
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
    // 根目录默认展开（但如果用户手动折叠了则以用户设置为准）
    let is_expanded = if depth == 0 {
        // 根目录：如果 HashMap 中没有记录，默认展开；否则使用用户的设置
        expanded.get(&node.path).copied().unwrap_or(true)
    } else {
        // 非根目录：默认折叠
        expanded.get(&node.path).copied().unwrap_or(false)
    };

    // 根据节点类型选择前缀图标
    let prefix = match node.node_type {
        NodeType::Directory => {
            if is_expanded {
                "📂 " // 已展开的文件夹
            } else {
                "📁 " // 未展开的文件夹
            }
        }
        NodeType::File => "📄 ",     // 普通文件
        NodeType::Symlink => "🔗 ",  // 符号链接
        NodeType::Skipped => "🚫 ",  // 跳过的路径
    };

    // 添加当前节点到列表
    items.push((
        format!("{}{}", prefix, node.name),
        crate::utils::format_size(node.size),
        depth,
        node.is_dir(),
        node.path.clone(),
    ));

    // 如果是目录且已展开，递归添加子节点
    if node.is_dir() && is_expanded {
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
