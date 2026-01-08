use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use retrace::Network;
use std::io;

mod app;
mod docs;
mod ui;
mod widgets;

use app::App;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    hash: Option<String>,

    #[arg(short, long, default_value = "mainnet")]
    network: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let args = Args::parse();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();

    if let Some(hash) = args.hash {
        let network = if args.network == "testnet" {
            Network::Testnet
        } else {
            Network::Mainnet
        };
        app.init_from_hash(network, &hash).await?;
    } else {
        // Default to a known hash if none provided, or just show empty
        let default_hash = "3c1b02a33390e596d83b306eab57b3f7271bc90e2e527ea4cafccfde25139d41";
        app.init_from_hash(Network::Mainnet, default_hash).await?;
    }

    // Run app loop
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| {
            ui::draw(f, app);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if app.show_docs {
                        if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
                            app.show_docs = false;
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Left | KeyCode::Up => app.on_up(),
                            KeyCode::Right | KeyCode::Down => app.on_down(),
                            KeyCode::Home => app.on_home(),
                            KeyCode::End => app.on_end(),
                            KeyCode::Enter => app.toggle_details(),
                            KeyCode::Char('?') => app.show_docs = true,
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    if !app.show_docs {
                        match mouse.kind {
                            MouseEventKind::ScrollUp => app.on_up(),
                            MouseEventKind::ScrollDown => app.on_down(),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
