#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
// use rocket::data::{Data, ToByteUnit};
// use rocket::http::uri::Absolute;
// use rocket::response::content::RawText;
use rocket::config::Config;
use rocket::fs::{FileName, NamedFile};
use rocket::http::Status;
use rocket::response::{content, status, Redirect};
use rocket::Request;
use rocket_dyn_templates::Template;

#[macro_use]
extern crate lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

use ar5iv::dirty_templates;

lazy_static! {
  static ref TRAILING_PDF_EXT: Regex = Regex::new("[.]pdf$").unwrap();
  static ref AR5IV_PAPERS_ROOT_DIR: String =
    env::var("AR5IV_PAPERS_ROOT_DIR").unwrap_or_else(|_| String::from("/data/arxmliv"));
}

#[get("/")]
async fn about() -> Template {
  let map: HashMap<String, String> = HashMap::new();
  Template::render("ar5iv", &map)
}

#[get("/favicon.ico")]
async fn favicon() -> Option<NamedFile> {
  NamedFile::open(Path::new("assets/").join("favicon.ico"))
    .await
    .ok()
}

#[get("/html/<id>")]
async fn get_html(id: String) -> content::RawHtml<String> {
  assemble_paper(None, id).await
}
#[get("/html/<field>/<id>")]
async fn get_field_html(field: String, id: String) -> content::RawHtml<String> {
  assemble_paper(Some(field), id).await
}

#[get("/html/<id>/assets/<filename>")]
async fn get_paper_asset(id: &str, filename: &str) -> Option<Vec<u8>> {
  assemble_paper_asset(None, id, filename)
}
#[get("/html/<field>/<id>/assets/<filename>", rank = 2)]
async fn get_field_paper_asset(field: String, id: &str, filename: &str) -> Option<Vec<u8>> {
  assemble_paper_asset(Some(field), id, filename)
}
#[get("/html/<id>/assets/<subdir>/<filename>")]
async fn get_paper_subdir_asset(id: &str, subdir: String, filename: &str) -> Option<Vec<u8>> {
  let compound_name = subdir + "/" + filename;
  assemble_paper_asset(None, id, &compound_name)
}
#[get("/html/<id>/assets/<subdir>/<subsubdir>/<filename>")]
async fn get_paper_subsubdir_asset(
  id: &str,
  subdir: String,
  subsubdir: &str,
  filename: &str,
) -> Option<Vec<u8>> {
  let compound_name = subdir + "/" + subsubdir + "/" + filename;
  assemble_paper_asset(None, id, &compound_name)
}
#[get("/html/<field>/<id>/assets/<subdir>/<filename>", rank = 2)]
async fn get_field_paper_subdir_asset(
  field: String,
  id: &str,
  subdir: String,
  filename: &str,
) -> Option<Vec<u8>> {
  let compound_name = subdir + "/" + filename;
  assemble_paper_asset(Some(field), id, &compound_name)
}
#[get("/html/<field>/<id>/assets/<subdir>/<subsubdir>/<filename>", rank = 2)]
async fn get_field_paper_subsubdir_asset(
  field: String,
  id: &str,
  subdir: String,
  subsubdir: &str,
  filename: &str,
) -> Option<Vec<u8>> {
  let compound_name = subdir + "/" + subsubdir + "/" + filename;
  assemble_paper_asset(Some(field), id, &compound_name)
}

#[get("/abs/<field>/<id>")]
async fn abs_field(field: &str, id: &str) -> Redirect {
  let to_uri = String::from("/html/") + field + "/" + id;
  Redirect::to(to_uri)
}
#[get("/abs/<id>")]
async fn abs(id: &str) -> Redirect {
  let to_uri = String::from("/html/") + id;
  Redirect::to(to_uri)
}

#[get("/pdf/<field>/<id>")]
async fn pdf_field(field: String, id: String) -> content::RawHtml<String> {
  let id_core: String = (*TRAILING_PDF_EXT.replace(&id, "")).to_owned();
  assemble_paper(Some(field), id_core).await
}
#[get("/pdf/<id>")]
async fn pdf(id: String) -> content::RawHtml<String> {
  let id_core: String = (*TRAILING_PDF_EXT.replace(&id, "")).to_owned();
  assemble_paper(None, id_core).await
}

#[get("/assets/<name>")]
async fn assets(name: String) -> Option<NamedFile> {
  NamedFile::open(Path::new("assets/").join(name)).await.ok()
}

#[catch(404)]
fn general_not_found() -> content::RawHtml<&'static str> {
  content::RawHtml(
    r#"
        <p>Hmm... What are you looking for?</p>
        Say <a href="/hello/Sergio/100">hello!</a>
    "#,
  )
}

#[catch(default)]
fn default_catcher(status: Status, req: &Request<'_>) -> status::Custom<String> {
  let msg = format!("{} ({})", status, req.uri());
  status::Custom(status, msg)
}

async fn assemble_paper(field_opt: Option<String>, id: String) -> content::RawHtml<String> {
  // Option<File>
  // TODO: Can the tokio::fs::File be swapped in here for some benefit? Does the ZIP crate allow for that?
  //       I couldn't easily understand the answer from what I found online.
  if let Some(paper_path) = build_paper_path(field_opt.as_ref(), &id) {
    let zipf = File::open(&paper_path).unwrap();
    let reader = BufReader::new(zipf);
    let mut zip = ZipArchive::new(reader).unwrap();

    let mut log = String::new();
    let mut html = String::new();
    for i in 0..zip.len() {
      if let Ok(mut file) = zip.by_index(i) {
        if file.is_file() {
          let mut asset = None;
          match file.name() {
            "cortex.log" => {
              file.read_to_string(&mut log).unwrap();
            }
            name if name.ends_with(".html") => {
              file.read_to_string(&mut html).unwrap();
            }
            other => {
              // record assets for later management4
              asset = Some(other.to_string());
            }
          }
          if let Some(asset_name) = asset {
            let mut file_contents = Vec::new();
            file.read_to_end(&mut file_contents).unwrap();
            // TODO: As soon as we get a cache engine added,
            //       the assets should be immediately inserted as we read the ZIP
            //       for async fetching by the browser in the /images/ routes
            // redis.insert(asset_name, file_contents);
          }
        }
      }
    }
    let id_arxiv = if let Some(ref field) = field_opt {
      format!("{}/{}", field, id)
    } else {
      id.to_owned()
    };
    content::RawHtml(dirty_templates::branded_ar5iv_html(html, log, id_arxiv))
  } else {
    content::RawHtml(format!(
      "paper id {}{} is not available on disk. ",
      field_opt.unwrap_or_default(),
      id
    ))
  }
}

fn assemble_paper_asset(field_opt: Option<String>, id: &str, filename: &str) -> Option<Vec<u8>> {
  if let Some(paper_path) = build_paper_path(field_opt.as_ref(), id) {
    let zipf = File::open(&paper_path).unwrap();
    let reader = BufReader::new(zipf);
    let mut zip = ZipArchive::new(reader).unwrap();
    if let Ok(mut asset) = zip.by_name(filename) {
      {
        let mut file_contents = Vec::new();
        asset.read_to_end(&mut file_contents).ok();
        return Some(file_contents);
      }
    }
    drop(zip);
  }
  None
}

fn build_paper_path(field_opt: Option<&String>, id: &str) -> Option<PathBuf> {
  let id_base = &id[0..4];
  let paper_path_str = format!(
    "{}/{}/{}{}/tex_to_html.zip",
    *AR5IV_PAPERS_ROOT_DIR,
    id_base,
    match field_opt {
      Some(s) => s,
      None => "",
    },
    id
  );
  let paper_path = Path::new(&paper_path_str);
  if paper_path.exists() {
    Some(paper_path.to_path_buf())
  } else {
    None
  }
}

#[launch]
fn rocket() -> _ {
  rocket::custom(Config::figment().merge(("template_dir", "templates")))
    .attach(Template::fairing())
    .mount(
      "/",
      routes![
        abs,
        abs_field,
        pdf,
        pdf_field,
        get_html,
        get_field_html,
        get_paper_asset,
        get_paper_subdir_asset,
        get_paper_subsubdir_asset,
        get_field_paper_asset,
        get_field_paper_subdir_asset,
        get_field_paper_subsubdir_asset,
        about,
        assets,
        favicon
      ],
    )
    .register("/", catchers![general_not_found, default_catcher])
}
