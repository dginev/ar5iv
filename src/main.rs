#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
use rocket::fs::NamedFile;
use rocket::http::ContentType;
use rocket::http::Status;
use rocket::response::{content, status, Redirect};
use rocket::{Request,State};
use rocket_db_pools::Connection;
use rocket_db_pools::Database;
use rocket_dyn_templates::Template;

use ar5iv::cache::{
  assemble_log_with_cache, assemble_paper_asset_with_cache, assemble_paper_with_cache, Cache,
  LuckyStore
};
use ar5iv::assemble_asset::{fetch_zip};
use ar5iv::constants::{AR5IV_CSS_URL,SITE_CSS_URL};
use std::collections::HashMap;
use std::path::Path;

#[macro_use]
extern crate lazy_static;
use regex::Regex;
lazy_static! {
  static ref TRAILING_PDF_EXT: Regex = Regex::new("[.]pdf$").unwrap();
  static ref TRAILING_ZIP_EXT: Regex = Regex::new("[.]zip$").unwrap();
}

fn default_context() -> HashMap<&'static str, &'static str> {
  let mut map: HashMap<&'static str, &'static str> = HashMap::new();
  map.insert("AR5IV_CSS_URL", AR5IV_CSS_URL);
  map.insert("SITE_CSS_URL", SITE_CSS_URL);
  map
}

#[get("/")]
async fn about() -> Template {
  Template::render("ar5iv", default_context())
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
) -> Result<content::RawHtml<String>, Redirect> {
  if let Some(paper) = assemble_paper_with_cache(conn, None, id, false).await {
    Ok(content::RawHtml(paper))
  } else {
    Err(Redirect::temporary(format!("https://arxiv.org/abs/{}",id)))
  }
}
#[get("/html/<field>/<id>")]
async fn get_field_html(
  conn: Option<Connection<Cache>>,
  field: &str,
  id: &str,
) -> Result<content::RawHtml<String>, Redirect> {
  if let Some(paper) = assemble_paper_with_cache(conn, Some(field), id, false).await {
    Ok(content::RawHtml(paper))
  } else {
    Err(Redirect::temporary(format!("https://arxiv.org/abs/{}/{}", field, id)))
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
#[get("/html/<id>/assets/<subdir>/<sub2dir>/<sub3dir>/<filename>")]
async fn get_paper_sub3dir_asset(
  conn: Option<Connection<Cache>>,
  id: &str,
  subdir: String,
  sub2dir: &str,
  sub3dir: &str,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  let compound_name = subdir + "/" + sub2dir + "/" + sub3dir + "/" + filename;
  assemble_paper_asset_with_cache(conn, None, id, &compound_name).await
}
#[get("/html/<id>/assets/<subdir>/<sub2dir>/<sub3dir>/<sub4dir>/<filename>")]
async fn get_paper_sub4dir_asset(
  conn: Option<Connection<Cache>>,
  id: &str,
  subdir: String,
  sub2dir: &str,
  sub3dir: &str,
  sub4dir: &str,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  let compound_name = subdir + "/" + sub2dir + "/" + sub3dir + "/" + sub4dir + "/" + filename;
  assemble_paper_asset_with_cache(conn, None, id, &compound_name).await
}
#[get("/html/<id>/assets/<subdir>/<sub2dir>/<sub3dir>/<sub4dir>/<sub5dir>/<filename>")]
#[allow(clippy::too_many_arguments)]
async fn get_paper_sub5dir_asset(
  conn: Option<Connection<Cache>>,
  id: &str,
  subdir: String,
  sub2dir: &str,
  sub3dir: &str,
  sub4dir: &str,
  sub5dir: &str,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  let compound_name = subdir + "/" + sub2dir + "/" + sub3dir + "/" + sub4dir + "/" + sub5dir + "/" + filename;
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

#[get("/html/<field>/<id>/assets/<subdir>/<sub2dir>/<sub3dir>/<filename>", rank = 2)]
async fn get_field_paper_sub3dir_asset(
  conn: Option<Connection<Cache>>,
  field: &str,
  id: &str,
  subdir: String,
  sub2dir: &str,
  sub3dir: &str,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  let compound_name = subdir + "/" + sub2dir + "/" + sub3dir + "/" + filename;
  assemble_paper_asset_with_cache(conn, Some(field), id, &compound_name).await
}

#[get("/html/<field>/<id>/assets/<subdir>/<sub2dir>/<sub3dir>/<sub4dir>/<filename>", rank = 2)]
#[allow(clippy::too_many_arguments)]
async fn get_field_paper_sub4dir_asset(
  conn: Option<Connection<Cache>>,
  field: &str,
  id: &str,
  subdir: String,
  sub2dir: &str,
  sub3dir: &str,
  sub4dir: &str,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  let compound_name = subdir + "/" + sub2dir + "/" + sub3dir + "/" + sub4dir + "/" + filename;
  assemble_paper_asset_with_cache(conn, Some(field), id, &compound_name).await
}

#[get(
  "/html/<field>/<id>/assets/<subdir>/<sub2dir>/<sub3dir>/<sub4dir>/<sub5dir>/<filename>",
  rank = 2
)]
#[allow(clippy::too_many_arguments)]
async fn get_field_paper_sub5dir_asset(
  conn: Option<Connection<Cache>>,
  field: &str,
  id: &str,
  subdir: String,
  sub2dir: &str,
  sub3dir: &str,
  sub4dir: &str,
  sub5dir: &str,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  let compound_name = subdir + "/" + sub2dir + "/" + sub3dir + "/" + sub4dir + "/" + sub5dir + "/" + filename;
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
#[get("/assets/fonts/<name>")]
async fn font_assets(name: String) -> Option<NamedFile> {
  NamedFile::open(Path::new("assets/fonts/").join(name)).await.ok()
}


#[catch(404)]
fn general_not_found(req: &Request) -> Template {
  let uri_id = req.uri().path().to_string();
  let mut map = default_context();
  map.insert("id", &uri_id[1..]);
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
    let mut map = default_context();
    map.insert("id", id);
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
    let mut map = default_context();
    let arxiv_id = format!("{}/{}", field, id);
    map.insert("id", &arxiv_id);
    
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

#[get("/feeling_lucky")]
async fn feeling_lucky(lucky_store: &State<LuckyStore>, conn_opt: Option<Connection<Cache>>) -> Redirect {
  if let Some(mut conn) = conn_opt {
    if let Some(uri) = lucky_store.inner().get(&mut conn).await {
      Redirect::to(String::from("/html/")+&uri)
    } else { // fallback to some standard paper
      Redirect::to("/html/1910.06709")
    } }
  else {
    Redirect::to("/html/1910.06709")
  }
}

#[get("/robots.txt")]
fn robots_txt() -> (ContentType, &'static str) {
  (ContentType::Plain, 
r###"User-agent: *
Disallow: /log/
"###) }

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
        get_paper_sub3dir_asset,
        get_paper_sub4dir_asset,
        get_paper_sub5dir_asset,
        get_field_paper_asset,
        get_field_paper_subdir_asset,
        get_field_paper_subsubdir_asset,
        get_field_paper_sub3dir_asset,
        get_field_paper_sub4dir_asset,
        get_field_paper_sub5dir_asset,
        about,
        assets,
        font_assets,
        favicon,
        feeling_lucky,
        robots_txt
      ],
    )
    .manage(LuckyStore::new())
    .register("/", catchers![general_not_found, default_catcher])
}
