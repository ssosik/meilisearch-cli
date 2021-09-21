use color_eyre::Report;
use meilisearch_cli::{event::Event, event::Events, Document};
use serde::{Deserialize, Serialize};
use std::io::{stdout, Write};
use termion::{event::Key, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use url::Url;

// Needed to provide `width()` method on String:
// no method named `width` found for struct `std::string::String` in the current scope
use unicode_width::UnicodeWidthStr;

/// TerminalApp holds the state of the application
pub(crate) struct TerminalApp {
    /// Current value of the query_input box
    pub(crate) query_input: String,
    /// Preview window
    pub(crate) output: String,
    /// Query Matches
    pub(crate) matches: Vec<Document>,
    /// Keep track of which matches are selected
    pub(crate) selected_state: ListState,
    /// Display the serialized payload to send to the server
    pub(crate) debug: String,
    /// Report the server response
    pub(crate) response: String,
}

impl TerminalApp {
    pub fn get_selected(&mut self) -> Vec<String> {
        let ret: Vec<String> = Vec::new();
        if let Some(_i) = self.selected_state.selected() {
            //if let Some(s) = self.matches[i].full_path.to_str() {
            //    ret.push(s.into());
            //}
        };
        ret
    }

    pub fn get_selected_contents(&mut self) -> String {
        if let Some(i) = self.selected_state.selected() {
            return self.matches[i].body.clone();
        };
        String::from("")
    }

    pub fn next(&mut self) {
        let i = match self.selected_state.selected() {
            Some(i) => {
                if i >= self.matches.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.selected_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.selected_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.matches.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.selected_state.select(Some(i));
    }
}

impl Default for TerminalApp {
    fn default() -> TerminalApp {
        TerminalApp {
            query_input: String::new(),
            output: String::new(),
            matches: Vec::new(),
            selected_state: ListState::default(),
            debug: String::new(),
            response: String::new(),
        }
    }
}

pub fn setup_panic() {
    std::panic::set_hook(Box::new(move |_x| {
        stdout()
            .into_raw_mode()
            .unwrap()
            .suspend_raw_mode()
            .unwrap();
        write!(
            stdout().into_raw_mode().unwrap(),
            "{}",
            termion::screen::ToMainScreen
        )
        .unwrap();
        // Clippy removed this line in favor of the println!("") below
        //write!(stdout(), "{:?}", x).unwrap();
        print!("");
    }));
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ApiQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    #[serde(rename = "q")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub sort: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    #[serde(rename = "facetsDistribution")]
    pub facets_distribution: Option<Vec<String>>,
}

/// Interactive query interface
pub fn query(client: reqwest::blocking::Client, uri: Url) -> Result<Vec<String>, Report> {
    let mut tui = tui::Terminal::new(TermionBackend::new(AlternateScreen::from(
        stdout().into_raw_mode().unwrap(),
    )))
    .unwrap();

    // Setup event handlers
    let events = Events::new();

    // Create default app state
    let mut app = TerminalApp::default();

    loop {
        // Draw UI
        tui.draw(|f| {
            let panes = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints(
                    [
                        Constraint::Min(1),
                        Constraint::Length(2),
                        Constraint::Length(2),
                        Constraint::Length(2),
                    ]
                    .as_ref(),
                )
                .split(f.size());
            let selected_style = Style::default().add_modifier(Modifier::REVERSED);

            let content = Layout::default()
                .direction(Direction::Horizontal)
                .margin(1)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(panes[0]);

            // Output area where match titles are displayed
            let matches: Vec<ListItem> = app
                .matches
                .iter()
                .map(|m| {
                    let content = vec![Spans::from(Span::raw(m.title.to_string()))];
                    ListItem::new(content)
                })
                .collect();
            let matches = List::new(matches)
                .block(Block::default().borders(Borders::LEFT))
                .highlight_style(selected_style)
                .highlight_symbol("> ");
            f.render_stateful_widget(matches, content[0], &mut app.selected_state);

            // Preview area where content is displayed
            let paragraph = Paragraph::new(app.output.as_ref())
                .block(Block::default().borders(Borders::ALL))
                .wrap(Wrap { trim: true });
            f.render_widget(paragraph, content[1]);

            // Input area where queries are entered
            let query_input = Paragraph::new(app.query_input.as_ref())
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::NONE));
            f.render_widget(query_input, panes[1]);

            // Make the cursor visible and ask tui-rs to put it at the specified
            // coordinates after rendering
            f.set_cursor(
                // Put cursor past the end of the query_input text
                panes[1].x + app.query_input.width() as u16,
                panes[1].y,
            );

            // Area to display the parsed Xapian::Query.get_description()
            let debug = Paragraph::new(app.debug.as_ref())
                .style(Style::default().fg(Color::Green))
                .block(Block::default().borders(Borders::NONE));
            f.render_widget(debug, panes[2]);

            // Area where errors are displayed, query parsing errors, etc
            let response = Paragraph::new(app.response.as_ref())
                .style(Style::default().fg(Color::Red))
                .block(Block::default().borders(Borders::NONE));
            f.render_widget(response, panes[3]);
        })?;

        // Handle input
        if let Event::Input(input) = events.next()? {
            match input {
                Key::Char('\n') => {
                    // Select choice
                    break;
                }
                Key::Ctrl('c') => {
                    break;
                }
                Key::Char(c) => {
                    app.query_input.push(c);
                }
                Key::Backspace => {
                    app.query_input.pop();
                }
                Key::Down | Key::Ctrl('n') => {
                    app.next();
                    app.output = app.get_selected_contents();
                }
                Key::Up | Key::Ctrl('p') => {
                    app.previous();
                    app.output = app.get_selected_contents();
                }
                _ => {}
            }

            //let mut inp: String = app.query_input.to_owned();

            let q = ApiQuery {
                query: Some(app.query_input.to_owned()),
                ..Default::default()
            };
            app.debug = serde_json::to_string(&q).unwrap();
            let res = client
                .post(uri.as_ref())
                .body(serde_json::to_string(&q).unwrap())
                .send()?;

            app.response = res.text()?;
        }
    }

    tui.clear().unwrap();

    Ok(app.get_selected())
}
