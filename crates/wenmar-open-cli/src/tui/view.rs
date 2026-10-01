//! Draws the one screen.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::app::{App, Screen};

/// Wenmar red, the one brand colour on the screen. The value is the brand
/// primary of Wenmar Pro's design tokens.
pub const WENMAR_RED: Color = Color::Rgb(0xe5, 0x09, 0x14);

/// The mark in front of what is typed.
const PROMPT: &str = "> ";

/// Draws the whole screen: the box to type in, the answer, and one line
/// saying where answers come from and which keys do what.
pub fn draw(frame: &mut Frame, app: &App) {
    let [top, middle, bottom] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    // The box to type in. The prompt mark is the only thing in colour.
    let mark = if app.no_color {
        Style::new().add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(WENMAR_RED).add_modifier(Modifier::BOLD)
    };
    // What fits is the end of the text, where the typing is.
    let room = usize::from(top.width).saturating_sub(2 + PROMPT.len() + 1);
    let typed: Vec<char> = app.input.chars().collect();
    let shown: String = typed
        .iter()
        .skip(typed.len().saturating_sub(room))
        .collect();
    let cursor = u16::try_from(shown.chars().count()).unwrap_or(u16::MAX);
    let input = Paragraph::new(Line::from(vec![
        Span::styled(PROMPT, mark),
        Span::raw(shown),
    ]))
    .block(Block::bordered().title(" Wenmar Open "));
    frame.render_widget(input, top);
    if top.width > 4 && top.height > 2 {
        let column = top
            .x
            .saturating_add(1)
            .saturating_add(PROMPT.len() as u16)
            .saturating_add(cursor)
            .min(top.x.saturating_add(top.width.saturating_sub(2)));
        frame.set_cursor_position(Position::new(column, top.y.saturating_add(1)));
    }

    // The answer.
    let style = match app.screen {
        Screen::Idle | Screen::Waiting => Style::new().add_modifier(Modifier::DIM),
        Screen::Answer(_) | Screen::Failed(_) => Style::new(),
    };
    let body: Vec<Line> = app
        .body()
        .into_iter()
        .skip(app.scroll)
        .map(Line::from)
        .collect();
    frame.render_widget(Paragraph::new(body).style(style), middle);

    // Where answers come from, and the keys.
    let keys = format!(
        " {} | Enter look up | Up/Down scroll | Esc clear, then quit",
        app.source
    );
    frame.render_widget(
        Paragraph::new(keys).style(Style::new().add_modifier(Modifier::DIM)),
        bottom,
    );
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use serde_json::json;

    use super::*;
    use crate::tui::app::Effect;

    /// Draws the app on a screen of the given size and returns its rows.
    fn rows(app: &App, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer.cell((x, y)).map_or(" ", |cell| cell.symbol()))
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }

    fn typed(app: &mut App, text: &str) {
        for character in text.chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }
    }

    #[test]
    fn the_first_screen_says_what_to_type() {
        let app = App::new("offline, data 2026.09".to_owned(), false);
        assert_eq!(
            rows(&app, 72, 8),
            [
                "┌ Wenmar Open ─────────────────────────────────────────────────────────┐",
                "│>                                                                     │",
                "└──────────────────────────────────────────────────────────────────────┘",
                "Type a VIN, or a vehicle such as 2019 civic si, and press Enter.",
                "",
                "A VIN is decoded. Anything else is looked up in the catalog.",
                "",
                " offline, data 2026.09 | Enter look up | Up/Down scroll | Esc clear, the",
            ]
        );
    }

    #[test]
    fn an_answer_fills_the_middle_and_scrolls() {
        let mut app = App::new("online, https://open.wenmarpro.com".to_owned(), false);
        typed(&mut app, "civic");
        let Effect::Ask { id, request } =
            app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        else {
            panic!("a lookup");
        };
        assert_eq!(rows(&app, 60, 6)[3], "Looking it up...");
        app.on_answer(
            id,
            &request,
            Ok(json!([
                { "id": "2020_honda_civic", "summary": "2020 Honda Civic, CVT, FWD, Sedan" },
                { "id": "2019_honda_civic", "summary": "2019 Honda Civic, FWD" }
            ])),
        );
        let screen = rows(&app, 60, 8);
        assert_eq!(
            screen[1],
            "│> civic                                                   │"
        );
        assert_eq!(screen[3], "ID                VEHICLE");
        assert_eq!(
            screen[4],
            "2020_honda_civic  2020 Honda Civic, CVT, FWD, Sedan"
        );
        assert_eq!(screen[5], "2019_honda_civic  2019 Honda Civic, FWD");
        app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(
            rows(&app, 60, 8)[3],
            "2020_honda_civic  2020 Honda Civic, CVT, FWD, Sedan"
        );
    }

    #[test]
    fn long_typing_shows_its_end() {
        let mut app = App::new(String::new(), false);
        typed(&mut app, &format!("{}END", "x".repeat(150)));
        let screen = rows(&app, 30, 5);
        assert!(screen[1].ends_with("xEND │"), "{:?}", screen[1]);
    }

    #[test]
    fn exactly_one_thing_on_the_screen_is_red_and_nothing_is_without_colour() {
        let count = |no_color: bool| {
            let mut app = App::new("offline".to_owned(), no_color);
            app.screen = Screen::Failed(vec!["error: no".to_owned()]);
            let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
            terminal.draw(|frame| draw(frame, &app)).unwrap();
            let buffer = terminal.backend().buffer();
            (0..8u16)
                .flat_map(|y| (0..40u16).map(move |x| (x, y)))
                .filter(|position| {
                    buffer
                        .cell(*position)
                        .is_some_and(|cell| cell.fg == WENMAR_RED && cell.symbol() != " ")
                })
                .count()
        };
        // The prompt mark, and only that.
        assert_eq!(count(false), 1);
        assert_eq!(count(true), 0);
    }

    #[test]
    fn a_screen_of_any_size_is_drawn_without_failing() {
        let mut app = App::new("offline, data 2026.09".to_owned(), false);
        typed(&mut app, "KM8K2CAB4PU001140");
        app.screen = Screen::Answer((0..100).map(|line| format!("line {line}")).collect());
        app.scroll = 99;
        for (width, height) in [(1, 1), (2, 2), (4, 3), (5, 4), (10, 1), (1, 10), (200, 60)] {
            let _ = rows(&app, width, height);
        }
        // Scrolled past the end by a smaller answer arriving: still drawn.
        app.screen = Screen::Answer(vec!["one".to_owned()]);
        let _ = rows(&app, 40, 10);
    }
}
