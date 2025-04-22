pub type Tui = ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>;

pub fn init() -> std::io::Result<Tui> {
    ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::terminal::EnterAlternateScreen
    )?;
    ratatui::crossterm::terminal::enable_raw_mode()?;
    set_panic_hook();
    ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout()))
}

fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore();
        hook(panic_info);
    }));
}

pub fn restore() -> std::io::Result<()> {
    ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::terminal::LeaveAlternateScreen
    )?;
    ratatui::crossterm::terminal::disable_raw_mode()?;
    Ok(())
}
