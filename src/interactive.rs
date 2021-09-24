use color_eyre::Report;
use eyre::bail;
use meilisearch_cli::{document, event::Event, event::Events};
use reqwest::header::CONTENT_TYPE;
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
use unicode_width::UnicodeWidthStr; // Provides `width()` method on String
use url::Url;

// TODO preview frontmatter in YAML not TOML
// TODO get server response debug area working
// TODO export documents with id/origid/latest into vimdiary git repo
// TODO V1 Uuids type
// TODO Syntax highlighting in preview pane with https://github.com/trishume/syntect

#[derive(Debug, Default, Serialize, Deserialize)]
struct ApiQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    #[serde(rename = "q")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub sort: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    #[serde(rename = "facetsDistribution")]
    pub facets_distribution: Option<Vec<String>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ApiResponse {
    pub hits: Vec<document::Document>,
    #[serde(rename = "nbHits")]
    pub num_hits: u32,
    #[serde(rename = "exhaustiveNbHits")]
    pub exhaustive_num_hits: bool,
    pub query: String,
    pub limit: u16,
    pub offset: u32,
    #[serde(rename = "processingTimeMs")]
    pub processing_time_ms: u32,
}

/// TerminalApp holds the state of the application
pub(crate) struct TerminalApp {
    /// Current value of the query_input box
    pub(crate) query_input: String,
    /// Current value of the filter_input box
    pub(crate) filter_input: String,
    /// Preview window
    pub(crate) output: String,
    /// Query Matches
    pub(crate) matches: Vec<document::Document>,
    /// Keep track of which matches are selected
    pub(crate) selected_state: ListState,
    /// Display error messages
    pub(crate) error: String,
    /// Display the serialized payload to send to the server
    pub(crate) debug: String,
    /// Report the server response
    pub(crate) response: String,
    // TODO Add fields for sort expression
    inp_idx: usize,
    // Length here should stay in sync with the number of editable areas
    inp_widths: [i32; 2],
}

impl TerminalApp {
    // TODO make this work for multiple selections
    pub fn get_selected(&mut self) -> Vec<String> {
        let ret: Vec<String> = Vec::new();
        if let Some(i) = self.selected_state.selected() {
            vec![self.matches[i].id.to_hyphenated().to_string()]
        } else {
            ret
        }
    }

    pub fn get_selected_contents(&mut self) -> String {
        match self.selected_state.selected() {
            Some(i) => self.matches[i].to_string(),
            None => String::from(""),
        }
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
            filter_input: String::new(),
            output: String::new(),
            matches: Vec::new(),
            selected_state: ListState::default(),
            error: String::new(),
            debug: String::new(),
            response: String::new(),
            inp_idx: 0,
            inp_widths: [0, 0],
        }
    }
}

pub fn setup_panic() {
    std::panic::set_hook(Box::new(move |x| {
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
        write!(stdout(), "{:?}", x).unwrap();
        print!("");
    }));
}

/// Interactive query interface
pub fn query(
    client: reqwest::blocking::Client,
    uri: Url,
    verbosity: u8,
) -> Result<Vec<String>, Report> {
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
        if let Err(e) = tui.draw(|f| {
            let main = if verbosity > 0 {
                // Enable debug and error output areas
                Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints(
                        [
                            // Content Preview Area
                            Constraint::Percentage(85),
                            // Server Response area
                            Constraint::Percentage(5),
                            // Debug Message Area
                            Constraint::Percentage(5),
                            // Error Message Area
                            Constraint::Percentage(5),
                        ]
                        .as_ref(),
                    )
                    .split(f.size())
            } else {
                Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([Constraint::Percentage(100)].as_ref())
                    .split(f.size())
            };

            let screen = Layout::default()
                .direction(Direction::Horizontal)
                .margin(1)
                .constraints(
                    [
                        // Match results area
                        Constraint::Percentage(50),
                        // Document Preview area
                        Constraint::Percentage(50),
                    ]
                    .as_ref(),
                )
                .split(main[0]);

            // Preview area where content is displayed
            let preview = Paragraph::new(app.output.as_ref())
                .block(Block::default().borders(Borders::ALL))
                .wrap(Wrap { trim: true });
            f.render_widget(preview, screen[1]);

            // Output area where match titles are displayed
            // TODO panes specifically for tags, weight, revisions, date, authors, id, origid,
            //    latest
            let interactive = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints(
                    [
                        // Match titles display area
                        Constraint::Min(20),
                        // Query input box
                        Constraint::Length(3),
                        // Filter input box
                        Constraint::Length(3),
                    ]
                    .as_ref(),
                )
                .split(screen[0]);

            let selected_style = Style::default().add_modifier(Modifier::REVERSED);
            let matches: Vec<ListItem> = app
                .matches
                .iter()
                .map(|m| ListItem::new(vec![Spans::from(Span::raw(m.title.to_string()))]))
                .collect();
            let matches = List::new(matches)
                .block(Block::default().borders(Borders::ALL))
                .highlight_style(selected_style)
                .highlight_symbol("> ");
            f.render_stateful_widget(matches, interactive[0], &mut app.selected_state);

            // Input area where queries are entered
            let query_input = Paragraph::new(app.query_input.as_ref())
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().title("Query input").borders(Borders::ALL));
            f.render_widget(query_input, interactive[1]);

            // Input area where filters are entered
            let filter_input = Paragraph::new(app.filter_input.as_ref())
                .style(Style::default().fg(Color::Yellow))
                .block(
                    Block::default()
                        .title("Filter input (e.g. 'tag=vim OR tag=bash')")
                        .borders(Borders::ALL),
                );
            f.render_widget(filter_input, interactive[2]);

            // Make the cursor visible and ask tui-rs to put it at the specified
            // coordinates after rendering
            f.set_cursor(
                // Put cursor past the end of the input text
                // TODO refactor input area switching
                interactive[app.inp_idx + 1].x + 1 + app.inp_widths[app.inp_idx] as u16,
                interactive[app.inp_idx + 1].y + 1,
            );

            if verbosity > 0 {
                // Area to display server response
                let response = Paragraph::new(app.response.as_ref())
                    .style(Style::default().fg(Color::White).bg(Color::Black))
                    .block(
                        Block::default()
                            .title("Server Response")
                            .borders(Borders::ALL),
                    );
                f.render_widget(response, main[1]);

                // Area to display debug messages
                let debug = Paragraph::new(app.debug.as_ref())
                    .style(Style::default().fg(Color::Green).bg(Color::Black))
                    .block(
                        Block::default()
                            .title("Debug messages")
                            .borders(Borders::ALL),
                    );
                f.render_widget(debug, main[2]);

                // Area to display Error messages
                let error = Paragraph::new(app.error.as_ref())
                    .style(Style::default().fg(Color::Red).bg(Color::Black))
                    .block(
                        Block::default()
                            .title("Error messages")
                            .borders(Borders::ALL),
                    );
                f.render_widget(error, main[3]);
            }
        }) {
            tui.clear().unwrap();
            drop(tui);
            bail!("Failed to draw TUI App {}", e.to_string());
        }
        //.expect("Failed to draw TUI App");

        // Handle input
        match events.next() {
            Err(e) => {
                tui.clear().unwrap();
                drop(tui);
                bail!("Failed to handle input {}", e.to_string());
            }
            Ok(ev) => {
                if let Event::Input(input) = ev {
                    //if let Event::Input(input) = events.next().expect("Failed to handle input") {

                    // TODO add support for:
                    //  - tab to switch between input boxes
                    //  - ctrl-e to open selected in $EDITOR, then submit on file close
                    //  - ctrl-v to open selected in $LESS
                    //  - pageup/pagedn/home/end for navigating displayed selection
                    //  - ctrl-jkdu for navigating displayed selection
                    //  - ctrl-hl for navigating between links
                    //  - Limit query and filter input box length
                    //  - +/- (and return) to modify weight
                    match input {
                        Key::Char('\n') => {
                            // Select choice
                            // TODO emit Doc ID
                            // TODO increment weight for selected doc
                            break;
                        }
                        Key::Ctrl('c') => {
                            break;
                        }
                        Key::Left | Key::Right | Key::Char('\t') => {
                            app.inp_idx = match app.inp_idx {
                                1 => 0,
                                _ => 1,
                            };
                        }
                        Key::Char(c) => {
                            if app.inp_idx == 0 {
                                app.query_input.push(c);
                            } else {
                                app.filter_input.push(c);
                            }
                            app.inp_widths[app.inp_idx] += 1;
                        }
                        Key::Backspace => {
                            if app.inp_idx == 0 {
                                app.query_input.pop();
                            } else {
                                app.filter_input.pop();
                            }
                            app.inp_widths[app.inp_idx] -= 1;
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

                    let mut q = ApiQuery {
                        query: Some(app.query_input.to_owned()),
                        ..Default::default()
                    };

                    let filter = app.filter_input.to_owned();
                    if filter.width() > 0 {
                        q.filter = Some(filter);
                    }

                    app.debug = serde_json::to_string(&q).unwrap();

                    // Split up the JSON decoding into two steps.
                    // 1.) Get the text of the body.
                    let response_body = match client
                        .post(uri.as_ref())
                        .body::<String>(serde_json::to_string(&q).unwrap())
                        .header(CONTENT_TYPE, "application/json")
                        .send()
                    {
                        Ok(resp) => {
                            if !resp.status().is_success() {
                                app.error = format!("Request failed: {:?}", resp);
                                continue;
                            }
                            match resp.text() {
                                Ok(text) => text,
                                Err(e) => {
                                    app.error = format!("resp.text() failed: {:?}", e);
                                    continue;
                                }
                            }
                        }
                        Err(e) => {
                            //papp.error = e.to_string();
                            app.error = format!("Send failed: {:?}", e);
                            continue;
                        }
                    };

                    // 2.) Parse the results as JSON.
                    match serde_json::from_str::<ApiResponse>(&response_body) {
                        Ok(resp) => {
                            app.matches = resp.hits;
                            app.error = String::from("");
                        }
                        Err(e) => {
                            app.error = format!(
                                "Could not deserialize body from: {}; error: {:?}",
                                response_body, e
                            )
                        }
                    };
                }
                //         Err(e) => {
                //             tui.clear().unwrap();
                //             drop(tui);
                //             bail!("Failed to POST request {}", e.to_string());
                //         }
                //     };
                // }
            }
        }
    }

    tui.clear().unwrap();

    Ok(app.get_selected())
}
