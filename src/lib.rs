use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Required fields
    pub id: Uuid,
    // If updating an existing doc, this will point to the `id` of the original document, and
    // the revision field should be incremented
    pub origid: Uuid,
    pub authors: Vec<String>,
    pub body: String,
    pub date: String,
    pub latest: bool,
    pub revision: u16,
    pub title: String,
    /// Optional fields
    #[serde(default)]
    pub background_img: String,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub weight: i32,
}

impl From<markdown_fm_doc::Document> for Document {
    fn from(item: markdown_fm_doc::Document) -> Self {
        //let uuid =Uuid::new_v4().to_hyphenated().to_string();
        let uuid = Uuid::new_v4();
        Document {
            id: uuid,
            origid: uuid,
            authors: vec![item.author],
            body: item.body,
            date: item.date,
            latest: true,
            revision: 1,
            tags: item.tags,
            title: item.title,
            subtitle: item.subtitle,
            ..Default::default()
        }
    }
}

pub mod event {

    use std::io;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use termion::event::Key;
    use termion::input::TermRead;

    pub enum Event<I> {
        Input(I),
        Tick,
    }

    /// A small event handler that wrap termion input and tick events. Each event
    /// type is handled in its own thread and returned to a common `Receiver`
    pub struct Events {
        rx: mpsc::Receiver<Event<Key>>,
        input_handle: thread::JoinHandle<()>,
        tick_handle: thread::JoinHandle<()>,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Config {
        pub tick_rate: Duration,
    }

    impl Default for Config {
        fn default() -> Config {
            Config {
                tick_rate: Duration::from_millis(250),
            }
        }
    }

    impl Events {
        pub fn new() -> Events {
            Events::with_config(Config::default())
        }

        pub fn with_config(config: Config) -> Events {
            let (tx, rx) = mpsc::channel();
            let input_handle = {
                let tx = tx.clone();
                thread::spawn(move || {
                    let stdin = io::stdin();
                    for evt in stdin.keys() {
                        if let Ok(key) = evt {
                            if let Err(err) = tx.send(Event::Input(key)) {
                                eprintln!("{}", err);
                                return;
                            }
                        }
                    }
                })
            };
            let tick_handle = {
                thread::spawn(move || loop {
                    if let Err(err) = tx.send(Event::Tick) {
                        eprintln!("{}", err);
                        break;
                    }
                    thread::sleep(config.tick_rate);
                })
            };
            Events {
                rx,
                input_handle,
                tick_handle,
            }
        }

        pub fn next(&self) -> Result<Event<Key>, mpsc::RecvError> {
            self.rx.recv()
        }
    }
}
