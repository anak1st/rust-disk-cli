mod app;
mod models;
mod scanner;
mod ui;
mod utils;

use app::App;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::env;
use std::path::Path;

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
            ui::render(f, &app);
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
