//! The terminal interface, driven key by key and drawn on a screen that is
//! only a buffer. No terminal is involved.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;
use wenmar_open_cli::backend::Backend;
use wenmar_open_cli::cli::Global;
use wenmar_open_cli::env::Env;
use wenmar_open_cli::tui::app::{App, Effect};
use wenmar_open_cli::tui::view;

use crate::common::{self, HOSTILE, KONA};
use crate::server::{self, Reply};

fn screen(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|frame| view::draw(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..30u16)
        .map(|y| {
            (0..100u16)
                .map(|x| buffer.cell((x, y)).map_or(" ", |cell| cell.symbol()))
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// Types the text, presses Enter, and has the lookup done at once, as the
/// worker thread would.
fn look_up(app: &mut App, backend: &Backend, text: &str) {
    while !app.input.is_empty() {
        app.on_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    }
    for character in text.chars() {
        app.on_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
    }
    let Effect::Ask { id, request } = app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
    else {
        panic!("Enter asks for a lookup");
    };
    let answer = backend.run(request.clone());
    app.on_answer(id, &request, answer);
}

#[test]
fn offline_a_vin_and_a_search_are_answered_from_the_data_file() {
    let fixture = common::data_dir();
    let backend =
        Backend::open(&common::env(&fixture), &Global::default(), &mut Vec::new()).unwrap();
    let mut app = App::new(backend.describe(), false);
    assert!(screen(&app).contains(" offline, data 2026.09 | Enter look up"));

    look_up(&mut app, &backend, KONA);
    let shown = screen(&app);
    assert!(shown.contains("2023 Hyundai Kona SE"), "{shown}");
    assert!(
        shown.contains("Vehicle id    2023_hyundai_kona_se_2-0l"),
        "{shown}"
    );

    look_up(&mut app, &backend, "2019 civic si");
    let shown = screen(&app);
    assert!(
        shown.contains("2019_honda_civic_si  2019 Honda Civic Si, Manual, FWD, Sedan"),
        "{shown}"
    );
    assert!(!shown.contains("Kona"), "{shown}");

    look_up(&mut app, &backend, "zzzz");
    assert!(screen(&app).contains("Nothing found."));

    // A mistyped VIN says what is wrong and what was probably meant.
    look_up(&mut app, &backend, "KM8K2CAB4PUO01140");
    let shown = screen(&app);
    assert!(
        shown.contains("error: a VIN uses only digits and letters other than I, O and Q"),
        "{shown}"
    );
    assert!(shown.contains("Did you mean: KM8K2CAB4PU001140"), "{shown}");
}

#[test]
fn control_characters_in_the_data_are_not_drawn() {
    let fixture = common::data_dir();
    let backend =
        Backend::open(&common::env(&fixture), &Global::default(), &mut Vec::new()).unwrap();
    let mut app = App::new(backend.describe(), false);
    look_up(&mut app, &backend, HOSTILE);
    let shown = screen(&app);
    assert!(shown.contains("Kona\u{fffd}[31m\u{fffd} Red"), "{shown}");
    assert!(!shown.contains('\u{1b}'));
}

#[test]
fn online_the_answer_comes_from_the_api_and_a_failure_is_shown_not_fatal() {
    let server = server::serve(|request| {
        if request.target.starts_with("/v1/vehicles/search") {
            Reply::json(
                200,
                &json!([{ "id": "2027_honda_civic", "summary": "2027 Honda Civic" }]),
            )
        } else {
            Reply::html(502, "<html>Bad gateway</html>")
        }
    });
    let fixture = common::empty_dir();
    let env = Env {
        api: Some(server.url().to_owned()),
        ..common::env(&fixture)
    };
    let backend = Backend::open(&env, &Global::default(), &mut Vec::new()).unwrap();
    let mut app = App::new(backend.describe(), false);
    assert!(screen(&app).contains(&format!(" online, {}", server.url())));

    look_up(&mut app, &backend, "civic");
    assert!(screen(&app).contains("2027_honda_civic  2027 Honda Civic"));

    look_up(&mut app, &backend, KONA);
    let shown = screen(&app);
    assert!(shown.contains("error: The answer from"), "{shown}");
    assert!(!shown.contains("Bad gateway"), "{shown}");
    // And the next lookup still works.
    look_up(&mut app, &backend, "civic");
    assert!(screen(&app).contains("2027 Honda Civic"));
}
