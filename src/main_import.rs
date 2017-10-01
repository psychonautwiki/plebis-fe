use std::fs::File;
use std::io::Read;

extern crate tantivy;
extern crate tempdir;
extern crate progress;

extern crate serde;

#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

extern crate exonum_leveldb;
extern crate leveldb_sys;

use exonum_leveldb::database::Database;
use exonum_leveldb::database::kv::KV;
use exonum_leveldb::iterator::Iterable;
use exonum_leveldb::options::{Options,WriteOptions,ReadOptions};

use std::path::Path;
use tempdir::TempDir;
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

fn main() {
    let dir = Path::new("./foo");
    let level_path = Path::new("./db");

    //run(dir);
    run_example(dir, level_path).unwrap();
}

fn run_example(index_path: &Path, level_path: &Path) -> tantivy::Result<()> {
    let mut schema_builder = SchemaBuilder::default();

    schema_builder.add_text_field("id", TEXT | STORED);
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT);

    let schema = schema_builder.build();

    let index = Index::create(index_path, schema.clone())?;

    let mut index_writer = index.writer(256_000_000)?;

    let id = schema.get_field("id").unwrap();
    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();

    let mut file = File::open("../dump.json").unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();

    let p: Vec<ReportData> = serde_json::from_str(&data).unwrap();

    let mut bar = progress::Bar::new();

    bar.set_job_title("Indexing...");

    let mut i: i32 = 0;

    for report in p.iter() {
        let foreign_id = report.meta.erowid_id.unwrap_or(0);

        if foreign_id == 0 {
            continue;
        }

        let foreign_id_string = foreign_id.to_string();

        let mut options = Options::new();

        options.compression = leveldb_sys::Compression::Snappy;
        options.create_if_missing = true;
        
        let database = Database::open(level_path, options).expect("failed to open database");

        let write_opts = WriteOptions::new();

        let report_json = serde_json::to_string(&report)?;

        let res = database.put(write_opts, &foreign_id_string, report_json.as_bytes());

        let mut doc = Document::default();
        doc.add_text(id, &foreign_id_string);
        doc.add_text(title, &report.title);
        doc.add_text(body, &report.body);

        index_writer.add_document(doc);

        if i % 100 == 0 {
            bar.reach_percent(i / (p.len() as i32 / 100));
        }

        i = i + 1;
    }

    bar.jobs_done();

    index_writer.commit()?;

    index_writer.wait_merging_threads()?;

    Ok(())
}
