//! The one-screen interface a bare `wenmar-open` opens at a terminal: type
//! a VIN or a vehicle, see the answer.
//!
//! [`app`] is the state and [`view`] draws it; neither needs a terminal.
//! This module is the part that does: it owns the terminal, reads keys,
//! and has lookups done on a second thread so that a slow answer from the
//! API never freezes the screen.

pub mod app;
pub mod view;

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event};
use serde_json::Value;

use crate::backend::Backend;
use crate::error::{CliError, IO};
use crate::request::Request;
use app::{App, Effect};

/// How long to wait for a key before looking for an answer.
const TICK: Duration = Duration::from_millis(50);

type Answer = (u64, Request, Result<Value, CliError>);

fn terminal_failed(error: &std::io::Error) -> CliError {
    CliError::new(IO, format!("the terminal could not be used: {error}"))
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    ask: &Sender<(u64, Request)>,
    answers: &Receiver<Answer>,
) -> Result<(), CliError> {
    loop {
        terminal
            .draw(|frame| view::draw(frame, app))
            .map_err(|error| terminal_failed(&error))?;
        while let Ok((id, request, answer)) = answers.try_recv() {
            app.on_answer(id, &request, answer);
        }
        if !event::poll(TICK).map_err(|error| terminal_failed(&error))? {
            continue;
        }
        if let Event::Key(key) = event::read().map_err(|error| terminal_failed(&error))? {
            match app.on_key(key) {
                Effect::Nothing => {}
                Effect::Quit => return Ok(()),
                Effect::Ask { id, request } => {
                    // The worker only stops when this loop does.
                    let _ = ask.send((id, request));
                }
            }
        }
    }
}

/// Opens the interface and returns when the person leaves it. The terminal
/// is put back as it was, whether this ends well or not.
pub fn run(backend: Backend, no_color: bool) -> Result<(), CliError> {
    let mut app = App::new(backend.describe(), no_color);
    let (ask, asked) = mpsc::channel::<(u64, Request)>();
    let (tell, answers) = mpsc::channel::<Answer>();
    // Lookups happen here, one at a time, in the order they were asked.
    std::thread::spawn(move || {
        for (id, request) in asked {
            let answer = backend.run(request.clone());
            if tell.send((id, request, answer)).is_err() {
                break;
            }
        }
    });

    let mut terminal = ratatui::try_init().map_err(|error| terminal_failed(&error))?;
    let outcome = event_loop(&mut terminal, &mut app, &ask, &answers);
    ratatui::restore();
    outcome
}
