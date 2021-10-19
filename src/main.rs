mod interactive;
mod query;
use color_eyre::Report;
use glob::{glob, Paths};
use meilisearch_cli::{api, document};
use reqwest::header::CONTENT_TYPE;
use std::fs;
use std::path::Path;
use structopt::StructOpt;
use url::Url;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "meilisearch-cli",
    about = "CLI interface to Meilisearch to storing and retrieving Zettelkasten-style notes",
    author = "Steve <steve@little-fluffy.cloud>"
)]
struct Opt {
    /// switch on verbosity
    #[structopt(short, long, parse(from_occurrences))]
    verbosity: u8,

    #[structopt(
        short,
        long,
        default_value = "http://127.0.0.1:7700",
        env = "MEILI_HOST"
    )]
    host: String,

    #[structopt(short, long, default_value = "", env = "MEILI_KEY")]
    key: String,

    #[structopt(short, long, default_value = "less", env = "PAGER")]
    pager: String,

    #[structopt(short, long, default_value = "vim", env = "EDITOR")]
    editor: String,

    #[structopt(subcommand)]
    subcmd: Subcommands,
}

#[derive(Debug, StructOpt)]
enum Subcommands {
    /// Import markdown-fm-doc formatted files matching the unexpanded glob pattern
    ImportLegacyMd { globpath: String },
    /// Import meilisearch-cli/Document formatted files matching the unexpanded glob pattern
    Import { globpath: String },
    /// Interactively query the server
    Query {},
    /// Non-interactive query, specify all parameters from the command line
    StaticQuery {
        #[structopt(default_value = "")]
        query: String,
        #[structopt(default_value = "")]
        filter: String,
    },
    /// Dump records to a local path
    Dump { path: String },
    /// Opens $EDITOR on a template and then adds it when the editor is closed
    New {},
    /// Adds TOML-based document
    Add {},
}

impl Opt {
    fn url(&self, path: &str) -> Url {
        let mut url = Url::parse(self.host.as_str()).unwrap();
        url.set_path(path);
        url
    }

    // TODO can I use a trait to define this function once for both Document and markdown_fm_doc?
    fn import(&self, path: &str) -> Result<(), Report> {
        let client = reqwest::blocking::Client::new();
        let url = self.url("indexes/notes/documents");
        // Read the markdown files and post them to local Meilisearch
        for entry in glob_files(path, self.verbosity).expect("Failed to read glob pattern") {
            match entry {
                Ok(path) => {
                    if let Ok(doc) = document::Document::parse_file(&path) {
                        let doc: Vec<document::Document> = vec![doc];
                        let res = client
                            .post(url.as_ref())
                            .body(serde_json::to_string(&doc).unwrap())
                            .send()?;
                        if self.verbosity > 0 {
                            println!("✅ {} {:?}", doc[0], res);
                        }
                    } else {
                        eprintln!("❌ Failed to load file {}", path.display());
                    }
                }

                Err(e) => eprintln!("❌ {:?}", e),
            }
        }
        Ok(())
    }

    fn legacy_import(&self, path: &str) -> Result<(), Report> {
        let client = reqwest::blocking::Client::new();
        let url = self.url("indexes/notes/documents");
        // Read the markdown files and post them to local Meilisearch
        for entry in glob_files(path, self.verbosity).expect("Failed to read glob pattern") {
            match entry {
                Ok(path) => {
                    if let Ok(mdfm_doc) = markdown_fm_doc::parse_file(&path) {
                        let doc: Vec<document::Document> = vec![mdfm_doc.into()];
                        let res = client
                            .post(url.as_ref())
                            .body(serde_json::to_string(&doc).unwrap())
                            .send()?;
                        if self.verbosity > 0 {
                            println!("✅ {} {:?}", doc[0], res);
                        }
                    } else {
                        eprintln!("❌ Failed to load file {}", path.display());
                    }
                }

                Err(e) => eprintln!("❌ {:?}", e),
            }
        }
        Ok(())
    }

    fn interactive_query(&self) -> Result<(), Report> {
        interactive::setup_panic();

        let client = reqwest::blocking::Client::new();
        let url = self.url("indexes/notes/search");
        match interactive::query(
            client,
            url,
            self.verbosity,
            self.pager.clone(),
            self.editor.clone(),
        ) {
            Ok(res) => {
                println!("Document IDs: {:?}", res);
            }
            Err(e) => {
                eprintln!("❌ {:?}", e);
                //std::panic::panic_any(e);
            }
        };
        Ok(())
    }

    fn static_query(&self, query: &str, filter: &str) -> Result<(), Report> {
        let client = reqwest::blocking::Client::new();
        let url = self.url("indexes/notes/search");
        match query::query(client, url, query.to_string(), filter.to_string()) {
            Ok(res) => {
                println!("Document IDs: {:?}", res);
            }
            Err(e) => {
                eprintln!("❌ {:?}", e);
                //std::panic::panic_any(e);
            }
        };
        Ok(())
    }

    fn dump(&self, path: &str) -> Result<(), Report> {
        fs::create_dir_all(path)?;

        let client = reqwest::blocking::Client::new();
        let url = self.url("indexes/notes/search");
        let q = api::ApiQuery::new();

        // Split up the JSON decoding into two steps.
        // 1.) Get the text of the body.
        let response_body = match client
            .post(url.as_ref())
            .body::<String>(serde_json::to_string(&q).unwrap())
            .header(CONTENT_TYPE, "application/json")
            .send()
        {
            Ok(resp) => {
                if !resp.status().is_success() {
                    eprintln!("Request failed: {:?}", resp);
                }
                match resp.text() {
                    Ok(text) => text,
                    Err(e) => {
                        eprintln!("resp.text() failed: {:?}", e);
                        String::from("")
                    }
                }
            }
            Err(e) => {
                eprintln!("Send failed: {:?}", e);
                String::from("")
            }
        };

        // 2.) Parse the results as JSON.
        match serde_json::from_str::<api::ApiResponse>(&response_body) {
            Ok(mut resp) => {
                for entry in resp
                    .hits
                    .iter_mut()
                    .map(|mut m| {
                        m.serialization_type = document::SerializationType::Disk;
                        m.to_owned()
                    })
                    .collect::<Vec<_>>()
                {
                    let f = Path::new(&path).join(&entry.filename);
                    fs::write(f, entry.to_string())?;
                }
            }
            Err(e) => {
                eprintln!("Response not OK: {:?}", e);
            }
        };
        Ok(())
    }
}

pub fn glob_files(source: &str, verbosity: u8) -> Result<Paths, Box<dyn std::error::Error>> {
    let glob_path = Path::new(&source);
    let glob_str = shellexpand::tilde(glob_path.to_str().unwrap());

    if verbosity > 0 {
        println!("Sourcing Markdown documents matching : {}", glob_str);
    }

    Ok(glob(&glob_str).expect("Failed to read glob pattern"))
}

fn setup() -> Result<(), Report> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
    }
    color_eyre::install()?;

    Ok(())
}

fn main() -> Result<(), Report> {
    setup()?;

    let opt = Opt::from_args();

    match opt.subcmd {
        Subcommands::Import { ref globpath } => opt.import(globpath),
        Subcommands::ImportLegacyMd { ref globpath } => opt.legacy_import(globpath),
        Subcommands::Query {} => opt.interactive_query(),
        Subcommands::Dump { ref path } => opt.dump(path),
        Subcommands::StaticQuery {
            ref query,
            ref filter,
        } => opt.static_query(query, filter),
        Subcommands::New {} => unimplemented!("not yet"),
        Subcommands::Add {} => unimplemented!("not yet"),
    }
}
