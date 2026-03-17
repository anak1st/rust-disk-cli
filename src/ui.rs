use crate::app::App;
use crate::utils::format_size;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

/// 渲染应用程序 UI
pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();

    // 数字宽度（固定）
    let size_width: usize = 12;
    // 左右边距各1
    let margin: usize = 1;
    // 列表区域的可用宽度
    let list_width = size.width as usize - margin * 2;

    // 计算列表区域高度
    // 总高度 - 顶部状态栏(3行) - 底部帮助栏(1行) - 列表边框(2行) = 可用内容高度
    let list_height = size.height.saturating_sub(6) as usize;

    // -------------------------------------------------------------------
    // 1. 绘制顶部状态栏
    // -------------------------------------------------------------------
    let status_bar = Block::default()
        .title("Disk Scanner - 磁盘空间分析工具")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::DarkGray));

    f.render_widget(status_bar, Rect::new(0, 0, size.width, 3));

    // 根据扫描状态显示不同信息
    let (status_text, time_text) = if app.state.is_scanning.load(std::sync::atomic::Ordering::Relaxed) {
        let files = app.state.files_scanned.load(std::sync::atomic::Ordering::Relaxed);
        let path = app.state.current_path.lock().map(|p| p.clone()).unwrap_or_default();
        (format!("扫描中: {} ({} 文件)", path, files), String::new())
    } else if let Some(ref root) = app.state.root {
        let size_text = format!("总大小: {}", format_size(root.size));
        let time_str = if app.state.scan_duration_ms > 0 {
            format!("{:.2}s", app.state.scan_duration_ms as f64 / 1000.0)
        } else {
            String::new()
        };
        (size_text, time_str)
    } else {
        ("未扫描".to_string(), String::new())
    };

    // 计算时间文本宽度，用于右对齐
    let time_width = time_text.len() as u16;

    // 左侧显示状态信息（留出时间显示的空间）
    let status_widget = Paragraph::new(status_text).style(Style::default().fg(Color::White));
    f.render_widget(status_widget, Rect::new(1, 1, size.width.saturating_sub(time_width + 2), 1));

    // 右上角显示耗时（如果有）
    if !time_text.is_empty() {
        let time_widget = Paragraph::new(time_text)
            .style(Style::default().fg(Color::Green));
        f.render_widget(time_widget, Rect::new(size.width.saturating_sub(time_width + 1), 1, time_width, 1));
    }

    // -------------------------------------------------------------------
    // 2. 绘制文件列表（带滚动）
    // -------------------------------------------------------------------
    // 只渲染可见区域的列表项
    let visible_items: Vec<ListItem> = app
        .list_items
        .iter()
        .skip(app.scroll_offset)
        .take(list_height)
        .enumerate()
        .map(|(i, (name, size_str, depth, _, _))| {
            let actual_index = app.scroll_offset + i;
            // 计算缩进（每个深度级别 2 个空格）
            let indent = "  ".repeat(*depth);
            let indent_width = depth * 2;

            // 可用宽度 = 总宽度 - 缩进 - 数字宽度 - 边距
            let max_name_width = list_width.saturating_sub(indent_width + size_width + margin);

            // 格式化显示内容：名称（根据深度动态调整）+ 大小（固定宽度）
            let truncated_name = if name.len() > max_name_width {
                format!("{}...", &name[..max_name_width.saturating_sub(3)])
            } else {
                name.clone()
            };
            let content = format!("{:<width$}{:>size$}", truncated_name, size_str, width = max_name_width, size = size_width);

            // 高亮当前选中的项
            let style = if actual_index == app.selected_index {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(format!("{}{}", indent, content)).style(style)
        })
        .collect();

    let list = List::new(visible_items)
        .block(Block::default().title("文件树").borders(Borders::ALL))
        .style(Style::default().bg(Color::Black));

    f.render_widget(list, Rect::new(0, 3, size.width, size.height.saturating_sub(4)));

    // -------------------------------------------------------------------
    // 3. 绘制底部帮助信息
    // -------------------------------------------------------------------
    let help_text = Paragraph::new("↑↓ 选择 | 空格 展开/折叠 | r 重新扫描 | q 退出")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(
        help_text,
        Rect::new(0, size.height.saturating_sub(1), size.width, 1),
    );

    // -------------------------------------------------------------------
    // 4. 绘制错误信息
    // -------------------------------------------------------------------
    if let Some(ref error) = app.state.error {
        let error_block = Paragraph::new(format!("错误: {}", error))
            .style(Style::default().fg(Color::Red).bg(Color::White));
        f.render_widget(
            error_block,
            Rect::new(0, size.height.saturating_sub(2), size.width, 1),
        );
    }
}
