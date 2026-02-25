mod app;
mod git;
mod highlight;
mod ui;

use std::io;
use std::panic;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind, MouseEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::event::{EnableMouseCapture, DisableMouseCapture};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::App;
use git::GitRepo;

fn main() {
    // Open git repo before terminal init so errors print normally
    let git_repo = match GitRepo::open(None) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            eprintln!("Run this command inside a git repository.");
            std::process::exit(1);
        }
    };

    // Set panic handler to restore terminal
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        original_hook(info);
    }));

    if let Err(e) = run(git_repo) {
        let _ = restore_terminal();
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run(git_repo: GitRepo) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(git_repo);

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.handle_key(key);
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollDown => app.scroll_diff_down(3),
                    MouseEventKind::ScrollUp => app.scroll_diff_up(3),
                    MouseEventKind::ScrollRight => app.scroll_diff_right(4),
                    MouseEventKind::ScrollLeft => app.scroll_diff_left(4),
                    _ => {}
                },
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }

    restore_terminal()?;
    Ok(())
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}
