#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
use rocket::fs::NamedFile;
use rocket::http::ContentType;
use rocket::http::Status;
use rocket::response::{content, status, Redirect};
use rocket::Request;
use rocket_db_pools::Connection;
use rocket_db_pools::Database;
use rocket_dyn_templates::Template;

use ar5iv::cache::{
  assemble_log_with_cache, assemble_paper_asset_with_cache, assemble_paper_with_cache, Cache,
};
use ar5iv::dirty_templates::{fetch_zip, AR5IV_CSS_URL};
use std::collections::HashMap;
use std::path::Path;

#[macro_use]
extern crate lazy_static;
use regex::Regex;
lazy_static! {
  static ref TRAILING_PDF_EXT: Regex = Regex::new("[.]pdf$").unwrap();
  static ref TRAILING_ZIP_EXT: Regex = Regex::new("[.]zip$").unwrap();
}

#[get("/")]
async fn about() -> Template {
  let mut map: HashMap<&'static str, &'static str> = HashMap::new();
  map.insert("AR5IV_CSS_URL", AR5IV_CSS_URL);
  Template::render("ar5iv", &map)
}

#[get("/favicon.ico")]
async fn favicon() -> Option<NamedFile> {
  NamedFile::open(Path::new("assets/").join("favicon.ico"))
    .await
    .ok()
}

#[get("/html/<id>")]
async fn get_html(
  conn: Option<Connection<Cache>>,
  id: &str,
) -> Result<content::RawHtml<String>, Template> {
  if let Some(paper) = assemble_paper_with_cache(conn, None, id).await {
    Ok(content::RawHtml(paper))
  } else {
    let mut map: HashMap<&str, &str> = HashMap::new();
    map.insert("id", id);
    map.insert("AR5IV_CSS_URL", AR5IV_CSS_URL);
    Err(Template::render("404", &map))
  }
}
#[get("/html/<field>/<id>")]
async fn get_field_html(
  conn: Option<Connection<Cache>>,
  field: &str,
  id: &str,
) -> Result<content::RawHtml<String>, Template> {
  if let Some(paper) = assemble_paper_with_cache(conn, Some(field), id).await {
    Ok(content::RawHtml(paper))
  } else {
    let mut map: HashMap<&str, &str> = HashMap::new();
    let arxiv_id = format!("{}/{}", field, id);
    map.insert("id", &arxiv_id);
    map.insert("AR5IV_CSS_URL", AR5IV_CSS_URL);
    Err(Template::render("404", &map))
  }
}

#[get("/html/<id>/assets/<filename>")]
async fn get_paper_asset(
  conn: Option<Connection<Cache>>,
  id: &str,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  assemble_paper_asset_with_cache(conn, None, id, filename).await
}
#[get("/html/<field>/<id>/assets/<filename>", rank = 2)]
async fn get_field_paper_asset(
  conn: Option<Connection<Cache>>,
  field: &str,
  id: &str,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  assemble_paper_asset_with_cache(conn, Some(field), id, filename).await
}
#[get("/html/<id>/assets/<subdir>/<filename>")]
async fn get_paper_subdir_asset(
  conn: Option<Connection<Cache>>,
  id: &str,
  subdir: String,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  let compound_name = subdir + "/" + filename;
  assemble_paper_asset_with_cache(conn, None, id, &compound_name).await
}
#[get("/html/<id>/assets/<subdir>/<subsubdir>/<filename>")]
async fn get_paper_subsubdir_asset(
  conn: Option<Connection<Cache>>,
  id: &str,
  subdir: String,
  subsubdir: &str,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  let compound_name = subdir + "/" + subsubdir + "/" + filename;
  assemble_paper_asset_with_cache(conn, None, id, &compound_name).await
}
#[get("/html/<field>/<id>/assets/<subdir>/<filename>", rank = 2)]
async fn get_field_paper_subdir_asset(
  conn: Option<Connection<Cache>>,
  field: &str,
  id: &str,
  subdir: String,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  let compound_name = subdir + "/" + filename;
  assemble_paper_asset_with_cache(conn, Some(field), id, &compound_name).await
}
#[get("/html/<field>/<id>/assets/<subdir>/<subsubdir>/<filename>", rank = 2)]
async fn get_field_paper_subsubdir_asset(
  conn: Option<Connection<Cache>>,
  field: &str,
  id: &str,
  subdir: String,
  subsubdir: &str,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  let compound_name = subdir + "/" + subsubdir + "/" + filename;
  assemble_paper_asset_with_cache(conn, Some(field), id, &compound_name).await
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

#[get("/papers/<field>/<id>")]
async fn vanity_style_field(field: &str, id: &str) -> Redirect {
  let to_uri = String::from("/html/") + field + "/" + id;
  Redirect::to(to_uri)
}
#[get("/papers/<id>")]
async fn vanity_style(id: &str) -> Redirect {
  let to_uri = String::from("/html/") + id;
  Redirect::to(to_uri)
}

#[get("/pdf/<field>/<id>")]
async fn pdf_field(field: &str, id: String) -> Redirect {
  let id_core: String = (*TRAILING_PDF_EXT.replace(&id, "")).to_owned();
  let to_uri = String::from("/html/") + field + "/" + &id_core;
  Redirect::to(to_uri)
}
#[get("/pdf/<id>")]
async fn pdf(id: String) -> Redirect {
  let id_core: String = (*TRAILING_PDF_EXT.replace(&id, "")).to_owned();
  let to_uri = String::from("/html/") + &id_core;
  Redirect::to(to_uri)
}

#[get("/assets/<name>")]
async fn assets(name: String) -> Option<NamedFile> {
  NamedFile::open(Path::new("assets/").join(name)).await.ok()
}

#[catch(404)]
fn general_not_found(req: &Request) -> Template {
  let mut map: HashMap<&str, &str> = HashMap::new();
  let uri_id = req.uri().path().to_string();
  map.insert("id", &uri_id[1..]);
  map.insert("AR5IV_CSS_URL", AR5IV_CSS_URL);
  Template::render("404", &map)
}

#[get("/log/<id>")]
async fn get_log(
  conn: Option<Connection<Cache>>,
  id: &str,
) -> Result<content::RawHtml<String>, Template> {
  if let Some(paper) = assemble_log_with_cache(conn, None, id).await {
    Ok(content::RawHtml(paper))
  } else {
    let mut map: HashMap<&str, &str> = HashMap::new();
    map.insert("id", id);
    map.insert("AR5IV_CSS_URL", AR5IV_CSS_URL);
    Err(Template::render("404", &map))
  }
}
#[get("/log/<field>/<id>")]
async fn get_field_log(
  conn: Option<Connection<Cache>>,
  field: &str,
  id: &str,
) -> Result<content::RawHtml<String>, Template> {
  if let Some(paper) = assemble_log_with_cache(conn, Some(field), id).await {
    Ok(content::RawHtml(paper))
  } else {
    let mut map: HashMap<&str, &str> = HashMap::new();
    let arxiv_id = format!("{}/{}", field, id);
    map.insert("id", &arxiv_id);
    map.insert("AR5IV_CSS_URL", AR5IV_CSS_URL);
    Err(Template::render("404", &map))
  }
}

#[get("/source/<id>")]
async fn get_source_zip(id: &str) -> Option<(ContentType, Vec<u8>)> {
  let id_core: String = (*TRAILING_ZIP_EXT.replace(id, "")).to_owned();
  fetch_zip(None, &id_core)
}
#[get("/source/<field>/<id>", rank = 2)]
async fn get_field_source_zip(field: &str, id: &str) -> Option<(ContentType, Vec<u8>)> {
  let id_core: String = (*TRAILING_ZIP_EXT.replace(id, "")).to_owned();
  fetch_zip(Some(field), &id_core)
}

#[catch(default)]
fn default_catcher(status: Status, req: &Request<'_>) -> status::Custom<String> {
  let msg = format!("{} ({})", status, req.uri());
  status::Custom(status, msg)
}

#[launch]
fn rocket() -> _ {
  rocket::build()
    .attach(Template::fairing())
    .attach(Cache::init())
    .mount(
      "/",
      routes![
        abs,
        abs_field,
        pdf,
        pdf_field,
        vanity_style,
        vanity_style_field,
        get_html,
        get_field_html,
        get_log,
        get_field_log,
        get_source_zip,
        get_field_source_zip,
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
