use color_eyre::Report;
use glob::{glob, Paths};
use meilisearch_cli::Document;
use std::path::Path;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "meilisearch-cli",
    about = "CLI interface to Meilisearch to storing and retrieving Zettelkasten-style notes",
    author = "Steve <steve@little-fluffy.cloud>"
)]
struct Opt {
    /// switch on verbosity
    #[structopt(short)]
    verbose: bool,

    #[structopt(subcommand)]
    import: MdImportCmd,
    // /// Activate debug mode
    //// short and long flags (-d, --debug) will be deduced from the field's name
    //#[structopt(short, long)]
    //debug: bool,

    ///// Set speed
    //// we don't want to name it "speed", need to look smart
    //#[structopt(short = "v", long = "velocity", default_value = "42")]
    //speed: f64,

    ///// Input file
    //#[structopt(parse(from_os_str))]
    //input: PathBuf,

    ///// Output file, stdout if not present
    //#[structopt(parse(from_os_str))]
    //output: Option<PathBuf>,

    ///// Where to write the output: to `stdout` or `file`
    //#[structopt(short)]
    //out_type: String,

    ///// File name: only required when `out-type` is set to `file`
    //#[structopt(name = "FILE", required_if("out-type", "file"))]
    //file_name: Option<String>,
}

#[derive(Debug, StructOpt)]
enum MdImportCmd {
    /// Unexpanded path and glob pattern
    Import { globpath: String },
}

pub fn glob_files(source: &str, verbosity: i8) -> Result<Paths, Box<dyn std::error::Error>> {
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
    let verbosity = cli.occurrences_of("v");

    if let Some(cli) = cli.subcommand_matches("import") {
        let client = reqwest::blocking::Client::new();

        // Read the markdown files and post them to local Meilisearch
        for entry in glob_files(cli.value_of("globpath").unwrap(), verbosity as i8)
            .expect("Failed to read glob pattern")
        {
            match entry {
                // TODO convert this to iterator style using map/filter
                Ok(path) => {
                    if let Ok(mdfm_doc) = markdown_fm_doc::parse_file(&path) {
                        let doc: Vec<Document> = vec![mdfm_doc.into()];
                        let res = client
                            .post("http://127.0.0.1:7700/indexes/notes/documents")
                            .body(serde_json::to_string(&doc).unwrap())
                            .send()?;
                        if verbosity > 0 {
                            println!("✅ {:?}", res,);
                        }
                    } else {
                        eprintln!("❌ Failed to load file {}", path.display());
                    }
                }

                Err(e) => eprintln!("❌ {:?}", e),
            }
        }
    }

    Ok(())
}
