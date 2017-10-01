#![feature(plugin, decl_macro, custom_derive)]
#![plugin(rocket_codegen)]

extern crate rocket;

extern crate rocket_contrib;

extern crate tantivy;

//#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

extern crate exonum_leveldb;
extern crate leveldb_sys;

#[macro_use] extern crate bart_derive;

use rocket::State;

use rocket::response::NamedFile;
use rocket::response::content::Html;

use rocket::request::{Form};

use rocket_contrib::Json;

use exonum_leveldb::database::Database;
use exonum_leveldb::database::kv::KV;

//use exonum_leveldb::iterator::Iterable;
// exonum_leveldb::options::WriteOptions,

use exonum_leveldb::options::{Options,ReadOptions};

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tantivy::Index;
use tantivy::schema::*;
use tantivy::collector::TopCollector;
use tantivy::query::QueryParser;

#[derive(Serialize, Deserialize, Debug)]
struct ReportDataMeta {
    year: Option<u32>,
    
    #[serde(rename = "erowidId")]
    erowid_id: Option<u32>,
    
    gender: Option<String>,
    age: Option<u32>,
    published: Option<String>,
    views: Option<u32>
}

#[derive(Serialize, Deserialize, Debug)]
struct ReportSubstanceInfo {
    amount: String,
    method: String,
    substance: String,
    form: String
}

#[derive(Serialize, Deserialize, Debug)]
struct ReportErowidNotes {
    caution: Vec<String>,
    note: Vec<String>,
    warning: Vec<String>
}

#[derive(Serialize, Deserialize, Debug)]
struct ReportData {
    title: String,
    substance: String,
    author: String,
    body: String,

    #[serde(rename = "substanceInfo")]
    substance_info: Vec<ReportSubstanceInfo>,
    meta: ReportDataMeta,

    #[serde(rename = "erowidNotes")]
    erowid_notes: ReportErowidNotes,

    #[serde(rename = "pullQuotes")]
    pull_quotes: Vec<String>
}

#[derive(Deserialize,Debug)]
struct SearchReq {
    query: String
}

#[get("/")]
fn static_index() -> io::Result<NamedFile> {
    NamedFile::open("static/index.html")
}

#[get("/<file..>")]
fn static_files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(file)).ok()
}

#[derive(BartDisplay)]
#[template = "templates/results.html"]
struct Results {
    titles: Vec<String>
}

#[derive(FromForm, Debug)]
struct FormQuery {
    q: String
}

#[get   ("/query?<req>")]
fn form_query(
    ctx: State<Arc<Mutex<(Index, QueryParser, (Schema, Field), Database)>>>,
    req: FormQuery
) -> tantivy::Result<Html<String>> {
    let ref search_ctx = *ctx.lock().unwrap();

    let query = Json(SearchReq {
        query: req.q.clone()
    });

    let reports = do_search(search_ctx, query)?;

    //println!("{:?}", req);
    //Ok(Json(reports))

    let x: Vec<String> = reports.iter().map(|report| report.title.clone()).collect();

    let results = &Results { titles: x };

    Ok(Html(results.to_string()))
}

#[post("/search", data = "<query>")]
fn search(
    ctx: State<Arc<Mutex<(Index, QueryParser, (Schema, Field), Database)>>>,
    query: Json<SearchReq>
) -> tantivy::Result<Json<Vec<ReportData>>> {
    let ref search_ctx = *ctx.lock().unwrap();
    let reports = do_search(search_ctx, query)?;

    println!("{:?}", reports);

    Ok(Json(reports))
}

fn do_search(
    ctx: &(Index, QueryParser, (Schema, Field), Database),
    query: Json<SearchReq>
) -> tantivy::Result<Vec<ReportData>> {
    let (ref index, ref query_parser, (ref _schema, ref id), ref database) = *ctx;

    let searcher = index.searcher();

    let query = query_parser.parse_query(&query.query)?;

    let mut top_collector = TopCollector::with_limit(10);

    searcher.search(&*query, &mut top_collector)?;

    let doc_addresses = top_collector.docs();

    let reports = doc_addresses
    .iter()
    .filter_map(|doc_address| {
        let retrieved_doc = searcher.doc(&doc_address);

        if !retrieved_doc.is_ok() {
            return None;
        }

        let retrieved_doc = retrieved_doc.unwrap();

        if let tantivy::schema::Value::Str(ref raw_id) = *retrieved_doc.get_all(*id)[0] {
            let read_opts = ReadOptions::new();

            let doc = database.get(read_opts, raw_id);

            if !doc.is_ok() {
                return None;
            }

            let doc = doc.unwrap();

            if !doc.is_some() {
                return None;
            }

            let doc = doc.unwrap();

            let docstr = std::str::from_utf8(&doc);

            if !docstr.is_ok() {
                return None;
            }

            let parsed_doc: ReportData = serde_json::from_str(docstr.unwrap()).unwrap();

            Some(parsed_doc)
        } else {
            None
        }
    })
    .collect();

    Ok(reports)
}

fn boot(index_path: &Path, level_path: &Path) -> tantivy::Result<()> {
    let mut schema_builder = SchemaBuilder::default();

    schema_builder.add_text_field("id", TEXT | STORED);
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT);

    let schema = schema_builder.build();

    let id = schema.get_field("id").unwrap();
    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();

    let index = Index::open(index_path)?;

    index.load_searchers()?;

    let query_parser = QueryParser::new(index.schema(), vec![id, title, body]);

    let mut options = Options::new();

    options.compression = leveldb_sys::Compression::No;
    
    let database = Database::open(level_path, options).expect("failed to open database");

    rocket::ignite()
        .manage(Arc::new(Mutex::new((index, query_parser, (schema, id), database))))
        .mount("/", routes![
            static_index,
            static_files,
            form_query,
            search
        ]).launch();

    Ok(())
}

fn main() {
    let index_path = Path::new("./foo");
    let level_path = Path::new("./db");

    boot(index_path, level_path).expect("y")
}
