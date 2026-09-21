use std::{
    io::{self, IsTerminal},
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
};
use tileboard::{
    app::App,
    config::{self, Config},
    metrics::Collector,
    tiles::Registry,
    ui,
};

#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Configuration file (defaults to the platform's user config directory).
    #[arg(short, long)]
    config: Option<PathBuf>,
    /// Print the default TOML configuration and exit.
    #[arg(long)]
    print_default_config: bool,
    /// Validate an existing configuration without opening the terminal UI.
    #[arg(long)]
    check: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.print_default_config {
        print!("{}", toml::to_string_pretty(&Config::default())?);
        return Ok(());
    }
    let path = args.config.map(Ok).unwrap_or_else(config::default_path)?;
    let registry = Registry::builtin();
    if args.check {
        let config = Config::load(&path, &registry)?;
        println!(
            "Valid: {} ({} profiles)",
            path.display(),
            config.profiles.len()
        );
        return Ok(());
    }
    ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        "Tileboard needs an interactive terminal. Use --check or --print-default-config for noninteractive use."
    );
    let config = config::load_or_create(&path, &registry)?;
    let mut app = App::new(config, path, registry);
    let collector = Collector::start();
    let mut terminal = ratatui::try_init().context("Opening terminal")?;
    let _guard = TerminalGuard;
    // Ratatui restores terminal modes on panic; also release our mouse capture.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(io::stdout(), DisableMouseCapture);
        previous(info);
    }));
    execute!(io::stdout(), EnableMouseCapture)?;
    while !app.quit {
        if let Some(metrics) = collector.latest() {
            app.update_metrics(metrics);
        }
        app.metrics.now = chrono::Local::now();
        terminal.draw(|frame| ui::draw(frame, &mut app))?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => app.handle_key(key),
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                Event::Resize(width, height) => {
                    app.resize(ratatui::layout::Rect::new(0, 0, width, height))
                }
                _ => {}
            }
        }
    }
    Ok(())
}

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableMouseCapture);
        ratatui::restore();
    }
}
