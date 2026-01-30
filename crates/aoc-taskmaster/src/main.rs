mod state;
mod theme;
mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    io,
    path::Path,
    sync::mpsc::{self, Receiver},
    time::Duration,
    time::Instant,
};

fn main() -> Result<()> {
    let root = state::resolve_root()?;
    let mut app = state::App::new(root);
    app.refresh(true);

    let (watcher, watch_rx) = setup_watcher(&app.root);
    let mut terminal = setup_terminal()?;
    app.sync_pane_title();
    let result = run_app(&mut terminal, &mut app, watch_rx);
    restore_terminal(&mut terminal)?;
    drop(watcher);

    if let Err(err) = result {
        eprintln!("aoc-taskmaster: {err}");
    }

    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut state::App,
    watch_rx: Option<Receiver<()>>,
) -> Result<()> {
    let tick_rate = if watch_rx.is_some() {
        Duration::from_secs(2)
    } else {
        Duration::from_millis(500)
    };
    let input_poll = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui::render(f, app))?;

        if event::poll(input_poll)? {
            match event::read()? {
                Event::Key(key) => {
                    if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                        app.handle_key(key);
                    }
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse(mouse);
                }
                Event::Resize(_, _) => {
                    app.mark_dirty();
                }
                _ => {}
            }
        }

        if let Some(rx) = &watch_rx {
            let mut changed = false;
            while rx.try_recv().is_ok() {
                changed = true;
            }
            if changed {
                app.refresh(true);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }

        if app.should_quit() {
            break;
        }
    }

    Ok(())
}

fn setup_watcher(root: &Path) -> (Option<RecommendedWatcher>, Option<Receiver<()>>) {
    let (tx, rx) = mpsc::sync_channel(1);
    let mut watcher = match RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            if res.is_ok() {
                let _ = tx.try_send(());
            }
        },
        Config::default(),
    ) {
        Ok(watcher) => watcher,
        Err(_) => return (None, None),
    };

    let mut watched = false;
    let state_dir = root.join(".taskmaster");
    let tasks_dir = root.join(".taskmaster/tasks");

    if state_dir.exists() {
        let _ = watcher.watch(&state_dir, RecursiveMode::NonRecursive);
        watched = true;
    }
    if tasks_dir.exists() {
        let _ = watcher.watch(&tasks_dir, RecursiveMode::NonRecursive);
        watched = true;
    }
    if !watched {
        let _ = watcher.watch(root, RecursiveMode::NonRecursive);
    }

    (Some(watcher), Some(rx))
}
