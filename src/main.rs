use color_eyre::Report;
use glob::{glob, Paths};
use meilisearch_cli::{api, document};
use reqwest::header::CONTENT_TYPE;
use std::fs;
use std::path::Path;
use structopt::StructOpt;
use url::Url;
mod interactive;

extern crate pest;
#[macro_use]
extern crate pest_derive;

use pest::{Parser, iterators as pest_iterators};

#[derive(Parser)]
#[grammar = "filter.pest"]
pub struct Filter;

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

    #[structopt(subcommand)]
    import: Subcommands,
}

#[derive(Debug, StructOpt)]
enum Subcommands {
    /// Import markdown-fm-doc formatted files matching the unexpanded glob pattern
    ImportLegacyMd { globpath: String },
    /// Import meilisearch-cli/Document formatted files matching the unexpanded glob pattern
    Import { globpath: String },
    /// Interactively query the server
    Query {},
    /// Dump records to a local path
    Dump { path: String },
    /// Opens $EDITOR on a template and then adds it when the editor is closed
    New {},
    /// Adds TOML-based document
    Add {},
    /// WIP pest
    Pest {},
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

    let cli = Opt::clap().get_matches();
    let verbosity = cli.occurrences_of("verbosity");
    let host = cli.value_of("host").unwrap();
    let _key = cli.value_of("key").unwrap();
    let url_base = Url::parse(host)?;

    if let Some(cli) = cli.subcommand_matches("import") {
        import(url_base, cli.value_of("globpath").unwrap(), verbosity as u8)?;
    } else if let Some(cli) = cli.subcommand_matches("import-legacy-md") {
        legacy_import(url_base, cli.value_of("globpath").unwrap(), verbosity as u8)?;
    } else if let Some(_cli) = cli.subcommand_matches("query") {
        interactive_query(url_base, verbosity as u8)?;
    } else if let Some(cli) = cli.subcommand_matches("dump") {
        dump(url_base, cli.value_of("path").unwrap(), verbosity as u8)?;
    } else if let Some(_cli) = cli.subcommand_matches("pest") {
        //let expr = Filter::parse(Rule::expression, "!foo || bar")
        let expr = Filter::parse(Rule::expression, "foo")
            .expect("unsuccessful parse") // unwrap the parse result
            .next()
            .unwrap();
        if let Ok(s) = process_filter(expr) {
            println!("FILTER: {}", s);
        }
        let expr = Filter::parse(Rule::expression, "!foo | bar + !qux")
            .expect("unsuccessful parse") // unwrap the parse result
            .next()
            .unwrap();
        if let Ok(s) = process_filter(expr) {
            println!("FILTER: {}", s);
        }
        let expr = Filter::parse(Rule::expression, "bar + !qux")
            .expect("unsuccessful parse") // unwrap the parse result
            .next()
            .unwrap();
        if let Ok(s) = process_filter(expr) {
            println!("FILTER: {}", s);
        }
    }

    Ok(())
}

type PestPair<'a> = pest_iterators::Pair<'a, Rule>;

fn process_filter(expr: PestPair<'_>) -> Result<String, Report> {
    let mut filter = String::from("");
    for t in expr.into_inner() {
        match t.as_rule() {
            Rule::tag => {
                filter.push_str("tag=");
                filter.push_str(t.as_str());
            }
            Rule::not_tag => {
                filter.push_str("tag!=");
                for i in t.into_inner() {
                    filter.push_str(i.as_str());
                }
            }
            Rule::operator => match t.into_inner().next().unwrap().as_rule() {
                Rule::and => {
                    filter.push_str(" AND ");
                }
                Rule::or => {
                    filter.push_str(" OR ");
                }
                _ => unreachable!(),
            },
            Rule::EOI => break,
            _ => unreachable!(),
        }
    }
    Ok(filter)
}

// TODO can I use a trait to define this function once for both Document and markdown_fm_doc?
fn import(mut url: Url, path: &str, verbosity: u8) -> Result<(), Report> {
    let client = reqwest::blocking::Client::new();
    url.set_path("indexes/notes/documents");
    // Read the markdown files and post them to local Meilisearch
    for entry in glob_files(path, verbosity as u8).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => {
                if let Ok(doc) = document::Document::parse_file(&path) {
                    let doc: Vec<document::Document> = vec![doc];
                    let res = client
                        .post(url.as_ref())
                        .body(serde_json::to_string(&doc).unwrap())
                        .send()?;
                    if verbosity > 0 {
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

fn legacy_import(mut url: Url, path: &str, verbosity: u8) -> Result<(), Report> {
    let client = reqwest::blocking::Client::new();
    url.set_path("indexes/notes/documents");
    // Read the markdown files and post them to local Meilisearch
    for entry in glob_files(path, verbosity as u8).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => {
                if let Ok(mdfm_doc) = markdown_fm_doc::parse_file(&path) {
                    let doc: Vec<document::Document> = vec![mdfm_doc.into()];
                    let res = client
                        .post(url.as_ref())
                        .body(serde_json::to_string(&doc).unwrap())
                        .send()?;
                    if verbosity > 0 {
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

fn interactive_query(mut url: Url, verbosity: u8) -> Result<(), Report> {
    interactive::setup_panic();

    let client = reqwest::blocking::Client::new();
    url.set_path("indexes/notes/search");
    match interactive::query(client, url, verbosity as u8) {
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

fn dump(mut url: Url, path: &str, _verbosity: u8) -> Result<(), Report> {
    fs::create_dir_all(path)?;

    let client = reqwest::blocking::Client::new();
    url.set_path("indexes/notes/search");
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
                    m.skip_serializing_body = true;
                    m.to_owned()
                })
                .collect::<Vec<_>>()
            {
                let f = Path::new(path).join(&entry.filename);
                fs::write(f, entry.to_string())?;
            }
        }
        Err(e) => {
            eprintln!("Response not OK: {:?}", e);
        }
    };
    Ok(())
}
