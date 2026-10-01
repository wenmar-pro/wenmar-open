//! The state of the one-screen interface. Nothing here touches a terminal
//! or a data source, so tests drive it key by key.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use serde_json::Value;
use wenmar_vin::Vin;

use crate::error::CliError;
use crate::render;
use crate::request::{Action, LONGEST_TEXT, Lookup, Request};

/// The fewest characters of a single word that are taken for a VIN someone
/// is part way through, or has mistyped.
const SHORTEST_VIN_ATTEMPT: usize = 11;

/// What the lower part of the screen shows.
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    /// Nothing has been looked up yet.
    Idle,
    /// A lookup is on its way.
    Waiting,
    /// An answer, as lines of text.
    Answer(Vec<String>),
    /// What went wrong, as lines of text.
    Failed(Vec<String>),
}

/// What the event loop must do after a key.
// An effect lives for one key press, so the size of a request in it costs
// nothing worth an allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Nothing,
    /// Look this up. `id` comes back with the answer.
    Ask {
        id: u64,
        request: Request,
    },
    Quit,
}

#[derive(Debug)]
pub struct App {
    /// What has been typed.
    pub input: String,
    pub screen: Screen,
    /// How many lines the answer is scrolled down.
    pub scroll: usize,
    /// Where answers come from, for the bottom line.
    pub source: String,
    /// Whether to leave colour out.
    pub no_color: bool,
    /// The id of the lookup whose answer is awaited.
    asked: u64,
}

/// The lookup some typed text means: a VIN, or a search of the catalog.
/// `None` for nothing but spaces.
pub fn request_for(input: &str) -> Option<Request> {
    let text = input.trim();
    if text.is_empty() {
        return None;
    }
    let one_word = !text.contains(char::is_whitespace);
    let stripped = wenmar_vin::vin::normalize(text);
    let like_a_vin = one_word
        && stripped.chars().count() >= SHORTEST_VIN_ATTEMPT
        && stripped
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
        && stripped.chars().any(|character| character.is_ascii_digit());
    if Vin::parse(text).is_ok() || like_a_vin {
        return Some(Request::VinDecode {
            vin: text.to_owned(),
            year: None,
        });
    }
    Some(Request::Vehicles {
        action: Action::Search,
        lookup: Lookup {
            query: Some(text.to_owned()),
            ..Lookup::default()
        },
    })
}

fn lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_owned).collect()
}

impl App {
    pub fn new(source: String, no_color: bool) -> App {
        App {
            input: String::new(),
            screen: Screen::Idle,
            scroll: 0,
            source,
            no_color,
            asked: 0,
        }
    }

    /// The lines the lower part of the screen holds.
    pub fn body(&self) -> Vec<String> {
        match &self.screen {
            Screen::Idle => lines(
                "Type a VIN, or a vehicle such as 2019 civic si, and press Enter.\n\nA VIN is decoded. Anything else is looked up in the catalog.",
            ),
            Screen::Waiting => vec!["Looking it up...".to_owned()],
            Screen::Answer(text) | Screen::Failed(text) => text.clone(),
        }
    }

    /// Takes one key.
    pub fn on_key(&mut self, key: KeyEvent) -> Effect {
        // A terminal may report a key's release as well as its press.
        if key.kind == KeyEventKind::Release {
            return Effect::Nothing;
        }
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('c' | 'd') if control => return Effect::Quit,
            KeyCode::Char('u') if control => self.input.clear(),
            KeyCode::Char(_) if control => {}
            KeyCode::Char(character) => {
                let room = self.input.chars().count() < LONGEST_TEXT;
                if room && !character.is_control() {
                    self.input.push(character);
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Esc => {
                if self.input.is_empty() {
                    return Effect::Quit;
                }
                self.input.clear();
            }
            KeyCode::Enter => {
                if let Some(request) = request_for(&self.input) {
                    self.asked = self.asked.wrapping_add(1);
                    self.screen = Screen::Waiting;
                    self.scroll = 0;
                    return Effect::Ask {
                        id: self.asked,
                        request,
                    };
                }
            }
            KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Down => self.scroll_by(1),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(10),
            KeyCode::PageDown => self.scroll_by(10),
            KeyCode::Home => self.scroll = 0,
            _ => {}
        }
        Effect::Nothing
    }

    /// Scrolls down, never past the last line.
    fn scroll_by(&mut self, lines: usize) {
        let last = self.body().len().saturating_sub(1);
        self.scroll = self.scroll.saturating_add(lines).min(last);
    }

    /// Takes the answer to a lookup. An answer to anything but the latest
    /// lookup is dropped: the person has asked for something else since.
    pub fn on_answer(&mut self, id: u64, request: &Request, answer: Result<Value, CliError>) {
        if id != self.asked || self.screen != Screen::Waiting {
            return;
        }
        self.scroll = 0;
        self.screen = match answer {
            Ok(value) => Screen::Answer(lines(&render::text(request, &value))),
            Err(error) => {
                let mut text = vec![format!("error: {}", render::clean(&error.message))];
                let suggestions: Vec<String> = error.details["suggestions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(render::clean)
                    .collect();
                if !suggestions.is_empty() {
                    text.push(format!("Did you mean: {}", suggestions.join(", ")));
                }
                if let Some(hint) = error.hint() {
                    text.push(render::clean(hint));
                }
                Screen::Failed(text)
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyEventState;
    use serde_json::json;

    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn typed(app: &mut App, text: &str) {
        for character in text.chars() {
            app.on_key(key(KeyCode::Char(character)));
        }
    }

    fn app() -> App {
        App::new("offline, data 2026.09".to_owned(), false)
    }

    #[test]
    fn a_vin_is_decoded_and_anything_else_is_searched_for() {
        let vin = |text: &str| {
            Some(Request::VinDecode {
                vin: text.to_owned(),
                year: None,
            })
        };
        let search = |text: &str| {
            Some(Request::Vehicles {
                action: Action::Search,
                lookup: Lookup {
                    query: Some(text.to_owned()),
                    ..Lookup::default()
                },
            })
        };
        for text in [
            "KM8K2CAB4PU001140",
            "km8k2cab4pu001140",
            "KM8-K2CAB4-PU001140",
            "KM8 K2CAB4 PU001140",
            // Mistyped or cut short: decoding it says what is wrong.
            "KM8K2CAB4PUO01140",
            "KM8K2CAB4PU00114",
            "KM8K2CAB4PU",
        ] {
            assert_eq!(request_for(text), vin(text), "{text}");
        }
        for text in [
            "2019 civic si",
            "chevy 1500",
            "f150",
            "silverado",
            "2019 honda civic touring",
            "mercedesbenz",
            "2019_honda_civic_si",
        ] {
            assert_eq!(request_for(text), search(text), "{text}");
        }
        assert_eq!(request_for("  f150  "), search("f150"));
        for text in ["", "   ", "\t"] {
            assert_eq!(request_for(text), None);
        }
    }

    #[test]
    fn typing_and_enter_ask_for_a_lookup() {
        let mut app = app();
        typed(&mut app, "f15x");
        app.on_key(key(KeyCode::Backspace));
        typed(&mut app, "0");
        assert_eq!(app.input, "f150");
        let effect = app.on_key(key(KeyCode::Enter));
        assert_eq!(
            effect,
            Effect::Ask {
                id: 1,
                request: request_for("f150").unwrap()
            }
        );
        assert_eq!(app.screen, Screen::Waiting);
        // What was typed stays, to be corrected.
        assert_eq!(app.input, "f150");
    }

    #[test]
    fn enter_on_nothing_does_nothing() {
        let mut app = app();
        assert_eq!(app.on_key(key(KeyCode::Enter)), Effect::Nothing);
        typed(&mut app, "   ");
        assert_eq!(app.on_key(key(KeyCode::Enter)), Effect::Nothing);
        assert_eq!(app.screen, Screen::Idle);
    }

    #[test]
    fn escape_clears_and_then_quits_and_control_c_quits_at_once() {
        let mut app = app();
        typed(&mut app, "f150");
        assert_eq!(app.on_key(key(KeyCode::Esc)), Effect::Nothing);
        assert_eq!(app.input, "");
        assert_eq!(app.on_key(key(KeyCode::Esc)), Effect::Quit);
        typed(&mut app, "f150");
        for letter in ['c', 'd'] {
            let chord = KeyEvent::new(KeyCode::Char(letter), KeyModifiers::CONTROL);
            assert_eq!(app.on_key(chord), Effect::Quit);
        }
        // A control chord is never typed into the box.
        app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL));
        assert_eq!(app.input, "f150");
        app.on_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert_eq!(app.input, "");
    }

    #[test]
    fn a_key_release_is_not_a_second_key() {
        let mut app = app();
        let release = KeyEvent::new_with_kind_and_state(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
            KeyEventState::NONE,
        );
        app.on_key(key(KeyCode::Char('a')));
        app.on_key(release);
        assert_eq!(app.input, "a");
    }

    #[test]
    fn the_input_has_a_longest_length_and_takes_no_control_characters() {
        let mut app = app();
        typed(&mut app, &"x".repeat(5_000));
        assert_eq!(app.input.chars().count(), 200);
        app.input.clear();
        typed(&mut app, "a\u{1b}b\u{7}c\n");
        assert_eq!(app.input, "abc");
        // Deleting from an empty box does nothing.
        app.input.clear();
        app.on_key(key(KeyCode::Backspace));
        assert_eq!(app.input, "");
    }

    #[test]
    fn an_answer_is_shown_and_a_late_one_is_dropped() {
        let mut app = app();
        typed(&mut app, "f150");
        let Effect::Ask { id: first, request } = app.on_key(key(KeyCode::Enter)) else {
            panic!("a lookup");
        };
        // Before it comes back, something else is asked for.
        let Effect::Ask { id: second, .. } = app.on_key(key(KeyCode::Enter)) else {
            panic!("a lookup");
        };
        let found = json!([{ "id": "2019_ford_f-150", "summary": "2019 Ford F-150, Pickup" }]);
        app.on_answer(first, &request, Ok(found.clone()));
        assert_eq!(app.screen, Screen::Waiting);
        app.on_answer(second, &request, Ok(found));
        assert_eq!(
            app.screen,
            Screen::Answer(vec![
                "ID               VEHICLE".to_owned(),
                "2019_ford_f-150  2019 Ford F-150, Pickup".to_owned()
            ])
        );
        // The same answer again changes nothing.
        app.on_answer(second, &request, Err(CliError::new("io", "late")));
        assert!(matches!(app.screen, Screen::Answer(_)));
    }

    #[test]
    fn an_error_is_shown_with_its_suggestions_and_its_hint() {
        let mut app = app();
        typed(&mut app, "KM8K2CAB4PUO01140");
        let Effect::Ask { id, request } = app.on_key(key(KeyCode::Enter)) else {
            panic!("a lookup");
        };
        let error = CliError::new(
            "invalid_vin",
            "a VIN uses only digits and letters other than I, O and Q",
        )
        .with_details(json!({ "suggestions": ["KM8K2CAB4PU001140"] }))
        .with_hint("Check\u{1b}[31m it.");
        app.on_answer(id, &request, Err(error));
        assert_eq!(
            app.screen,
            Screen::Failed(vec![
                "error: a VIN uses only digits and letters other than I, O and Q".to_owned(),
                "Did you mean: KM8K2CAB4PU001140".to_owned(),
                "Check\u{fffd}[31m it.".to_owned()
            ])
        );
    }

    #[test]
    fn scrolling_stays_inside_the_answer() {
        let mut app = app();
        app.screen = Screen::Answer((0..5).map(|line| line.to_string()).collect());
        app.on_key(key(KeyCode::Up));
        assert_eq!(app.scroll, 0);
        for _ in 0..3 {
            app.on_key(key(KeyCode::PageDown));
        }
        assert_eq!(app.scroll, 4);
        app.on_key(key(KeyCode::Down));
        assert_eq!(app.scroll, 4);
        app.on_key(key(KeyCode::PageUp));
        assert_eq!(app.scroll, 0);
        app.on_key(key(KeyCode::Down));
        app.on_key(key(KeyCode::Home));
        assert_eq!(app.scroll, 0);
    }
}
