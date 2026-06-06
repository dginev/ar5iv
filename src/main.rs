#[macro_use]
extern crate rocket;
use rocket::fs::NamedFile;
use rocket::http::ContentType;
use rocket::http::Header;
use rocket::http::Status;
use rocket::response::{self, content, status, Redirect, Responder};
use rocket::{Request, State};
use rocket_db_pools::Connection;
use rocket_db_pools::Database;
use rocket_dyn_templates::Template;

use ar5iv::assemble_asset::fetch_zip;
use ar5iv::cache::{
  assemble_log_with_cache, assemble_paper_asset_with_cache, assemble_paper_with_cache, Cache,
  LuckyStore,
};
use ar5iv::constants::{AR5IV_CSS_URL, AR5IV_FONTS_CSS_URL, SITE_CSS_URL};
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

// jemalloc returns freed memory to the OS (with background purging via
// _RJEM_MALLOC_CONF), avoiding the glibc-malloc arena retention that ratchets
// RSS up under the large transient allocations of cache-miss paper assembly.
#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

static TRAILING_PDF_EXT: LazyLock<Regex> = LazyLock::new(|| Regex::new("[.]pdf$").unwrap());
static TRAILING_ZIP_EXT: LazyLock<Regex> = LazyLock::new(|| Regex::new("[.]zip$").unwrap());

/// The modern arXiv id scheme (since 2007-04): YYMM.NNNN(N), optional version.
static NEW_STYLE_ID: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^\d{2}(0[1-9]|1[0-2])\.\d{4,5}(v\d{1,2})?$").unwrap());
/// The number part of the legacy scheme (1991-07 .. 2007-03): YYMMNNN, optional
/// version. (We allow all of 2007 rather than encoding the mid-year cutoff;
/// arxiv.org is the final arbiter for those few months.)
static OLD_STYLE_NUMBER: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^(9[1-9]|0[0-7])(0[1-9]|1[0-2])\d{3}(v\d{1,2})?$").unwrap());

/// The legacy arXiv archive names -- a closed set, frozen at the 2007-04
/// identifier scheme change. See https://arxiv.org/help/arxiv_identifier
const OLD_STYLE_ARCHIVES: &[&str] = &[
  "acc-phys", "adap-org", "alg-geom", "ao-sci", "astro-ph", "atom-ph", "bayes-an", "chao-dyn",
  "chem-ph", "cmp-lg", "comp-gas", "cond-mat", "cs", "dg-ga", "funct-an", "gr-qc", "hep-ex",
  "hep-lat", "hep-ph", "hep-th", "math", "math-ph", "mtrl-th", "nlin", "nucl-ex", "nucl-th",
  "patt-sol", "physics", "plasm-ph", "q-alg", "q-bio", "quant-ph", "solv-int", "supr-con",
];

/// Only redirect to arxiv.org for ids matching one of its known identifier
/// schemes; everything else gets our 404 page rather than forwarding junk
/// traffic (crawler typos, spam probes) to arXiv.
fn is_plausible_arxiv_id(field_opt: Option<&str>, id: &str) -> bool {
  match field_opt {
    // modern scheme, e.g. "2105.04404" or "0704.0001v2"
    None => NEW_STYLE_ID.is_match(id),
    // legacy scheme, e.g. "math/0211159" or "math.GT/0309136"
    Some(field) => {
      let (archive, subject_opt) = match field.split_once('.') {
        Some((archive, subject)) => (archive, Some(subject)),
        None => (field, None),
      };
      OLD_STYLE_ARCHIVES.contains(&archive)
        && subject_opt.is_none_or(|subject| {
          (2..=9).contains(&subject.len())
            && subject.bytes().all(|b| b.is_ascii_alphabetic() || b == b'-')
        })
        && OLD_STYLE_NUMBER.is_match(id)
    }
  }
}

/// Fallback responses for /html/ requests we cannot serve locally:
/// plausible arXiv ids are forwarded to arxiv.org, the rest get a 404.
#[derive(Responder)]
// Neither variant is boxed: the two are nearly the same size (so no stack is
// wasted on the smaller one), and the value is built once per fallback response
// and immediately consumed -- boxing would only add a heap allocation.
enum HtmlFallback {
  Redirect(Redirect),
  #[response(status = 404)]
  NotFound(Template),
}

/// Cache-Control values: versioned site assets are immutable; paper pages and
/// their assets only change on (rare) reprocessing, so modest lifetimes are safe.
const CC_IMMUTABLE: &str = "public, max-age=31536000, immutable";
const CC_PAPER: &str = "public, max-age=3600";
const CC_PAPER_ASSET: &str = "public, max-age=86400";

/// Wraps any responder, adding a Cache-Control header.
struct CacheControlled<R>(R, &'static str);
impl<'r, 'o: 'r, R: Responder<'r, 'o>> Responder<'r, 'o> for CacheControlled<R> {
  fn respond_to(self, req: &'r Request<'_>) -> response::Result<'o> {
    let mut resp = self.0.respond_to(req)?;
    resp.set_header(Header::new("Cache-Control", self.1));
    Ok(resp)
  }
}

/// Percent-encode an untrusted id for safe inclusion in a redirect Location;
/// a raw non-ASCII byte would make the URI invalid and fail the responder.
fn percent_encode_id(id: &str) -> String {
  let mut out = String::with_capacity(id.len());
  for b in id.bytes() {
    match b {
      b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
        out.push(b as char)
      }
      _ => out.push_str(&format!("%{b:02X}")),
    }
  }
  out
}

fn default_context() -> HashMap<&'static str, &'static str> {
  let mut map: HashMap<&'static str, &'static str> = HashMap::new();
  map.insert("AR5IV_FONTS_CSS_URL", AR5IV_FONTS_CSS_URL);
  map.insert("AR5IV_CSS_URL", AR5IV_CSS_URL);
  map.insert("SITE_CSS_URL", SITE_CSS_URL);
  map
}

#[get("/")]
async fn about() -> Template {
  Template::render("ar5iv", default_context())
}

#[get("/favicon.ico")]
async fn favicon() -> Option<CacheControlled<NamedFile>> {
  NamedFile::open(Path::new("assets/").join("favicon.ico"))
    .await
    .ok()
    .map(|f| CacheControlled(f, CC_IMMUTABLE))
}

#[get("/html/<id>")]
async fn get_html(
  conn: Option<Connection<Cache>>,
  id: &str,
) -> Result<CacheControlled<content::RawHtml<String>>, HtmlFallback> {
  if let Some(paper) = assemble_paper_with_cache(conn, None, id).await {
    Ok(CacheControlled(content::RawHtml(paper), CC_PAPER))
  } else if is_plausible_arxiv_id(None, id) {
    Err(HtmlFallback::Redirect(Redirect::temporary(format!(
      "https://arxiv.org/abs/{}",
      percent_encode_id(id)
    ))))
  } else {
    let mut map = default_context();
    map.insert("id", id);
    Err(HtmlFallback::NotFound(Template::render("404", &map)))
  }
}
#[get("/html/<field>/<id>", rank = 2)]
async fn get_field_html(
  conn: Option<Connection<Cache>>,
  field: &str,
  id: &str,
) -> Result<CacheControlled<content::RawHtml<String>>, HtmlFallback> {
  if let Some(paper) = assemble_paper_with_cache(conn, Some(field), id).await {
    Ok(CacheControlled(content::RawHtml(paper), CC_PAPER))
  } else if is_plausible_arxiv_id(Some(field), id) {
    Err(HtmlFallback::Redirect(Redirect::temporary(format!(
      "https://arxiv.org/abs/{}/{}",
      percent_encode_id(field),
      percent_encode_id(id)
    ))))
  } else {
    let mut map = default_context();
    let arxiv_id = format!("{field}/{id}");
    map.insert("id", &arxiv_id);
    Err(HtmlFallback::NotFound(Template::render("404", &map)))
  }
}

#[get("/html/<id>/assets/<path..>", rank = 3)]
async fn get_paper_asset(
  conn: Option<Connection<Cache>>,
  id: &str,
  path: PathBuf,
) -> Result<CacheControlled<(ContentType, Vec<u8>)>, Option<NamedFile>> {
  let filename = path.to_string_lossy();
  assemble_paper_asset_with_cache(conn, None, id, &filename)
    .await
    .map(|asset| CacheControlled(asset, CC_PAPER_ASSET))
}
#[get("/html/<field>/<id>/assets/<path..>", rank = 4)]
async fn get_field_paper_asset(
  conn: Option<Connection<Cache>>,
  field: &str,
  id: &str,
  path: PathBuf,
) -> Result<CacheControlled<(ContentType, Vec<u8>)>, Option<NamedFile>> {
  let filename = path.to_string_lossy();
  assemble_paper_asset_with_cache(conn, Some(field), id, &filename)
    .await
    .map(|asset| CacheControlled(asset, CC_PAPER_ASSET))
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
async fn assets(name: &str) -> Option<CacheControlled<NamedFile>> {
  NamedFile::open(Path::new("assets/").join(name))
    .await
    .ok()
    .map(|f| CacheControlled(f, CC_IMMUTABLE))
}
#[get("/assets/fonts/<name>")]
async fn font_assets(name: &str) -> Option<CacheControlled<NamedFile>> {
  NamedFile::open(Path::new("assets/fonts/").join(name))
    .await
    .ok()
    .map(|f| CacheControlled(f, CC_IMMUTABLE))
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
    let arxiv_id = format!("{field}/{id}");
    map.insert("id", &arxiv_id);

    Err(Template::render("404", &map))
  }
}

#[get("/source/<id>")]
async fn get_source_zip(id: &str) -> Option<NamedFile> {
  let id_core: String = (*TRAILING_ZIP_EXT.replace(id, "")).to_owned();
  fetch_zip(None, &id_core).await
}
#[get("/source/<field>/<id>", rank = 2)]
async fn get_field_source_zip(field: &str, id: &str) -> Option<NamedFile> {
  let id_core: String = (*TRAILING_ZIP_EXT.replace(id, "")).to_owned();
  fetch_zip(Some(field), &id_core).await
}

#[get("/feeling_lucky")]
async fn feeling_lucky(
  lucky_store: &State<LuckyStore>,
  conn_opt: Option<Connection<Cache>>,
) -> Redirect {
  if let Some(mut conn) = conn_opt {
    if let Some(uri) = lucky_store.inner().get(&mut conn).await {
      Redirect::to(String::from("/html/") + &uri)
    } else {
      // fallback to some standard paper
      Redirect::to("/html/1910.06709")
    }
  } else {
    Redirect::to("/html/1910.06709")
  }
}

#[get("/robots.txt")]
fn robots_txt() -> (ContentType, &'static str) {
  (
    ContentType::Plain,
    r###"User-agent: *
Disallow: /log/
"###,
  )
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
        get_field_paper_asset,
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

#[cfg(test)]
mod tests {
  use rocket::http::Status;
  use rocket::local::blocking::Client;

  fn client() -> Client {
    Client::tracked(super::rocket()).expect("valid rocket instance")
  }

  #[test]
  fn landing_page_renders() {
    let client = client();
    let response = client.get("/").dispatch();
    assert_eq!(response.status(), Status::Ok);
  }

  #[test]
  fn short_source_id_is_a_clean_404() {
    // regression: `/source/abc` used to panic on `&id[0..4]`
    let client = client();
    let response = client.get("/source/abc").dispatch();
    assert_eq!(response.status(), Status::NotFound);
  }

  #[test]
  fn multibyte_id_is_a_clean_404() {
    // regression: ids where byte 4 splits a UTF-8 char used to panic
    // ("ab€cd", with the euro sign percent-encoded); since they cannot
    // be arXiv ids, they 404 rather than redirect.
    let client = client();
    let response = client.get("/html/ab%E2%82%ACcd").dispatch();
    assert_eq!(response.status(), Status::NotFound);
  }

  #[test]
  fn malformed_ids_are_not_relayed_to_arxiv() {
    let client = client();
    for uri in [
      "/html/blahblah",
      "/html/9999.99999",         // month 99 cannot exist
      "/html/2105.04404extra",
      "/html/notanarchive/0211159",
      "/html/math/021115",        // six digits
      "/html/math/0213159",       // month 13 cannot exist
    ] {
      let response = client.get(uri).dispatch();
      assert_eq!(response.status(), Status::NotFound, "expected 404 for {uri}");
    }
  }

  #[test]
  fn plausible_unknown_papers_redirect_to_arxiv_abs() {
    let client = client();
    let response = client.get("/html/2512.99999").dispatch();
    assert_eq!(response.status(), Status::TemporaryRedirect);
    assert_eq!(
      response.headers().get_one("Location"),
      Some("https://arxiv.org/abs/2512.99999")
    );
    let response = client.get("/html/math/0211159v2").dispatch();
    assert_eq!(response.status(), Status::TemporaryRedirect);
    assert_eq!(
      response.headers().get_one("Location"),
      Some("https://arxiv.org/abs/math/0211159v2")
    );
  }

  #[test]
  fn arxiv_id_scheme_validation() {
    use super::is_plausible_arxiv_id;
    // modern scheme
    assert!(is_plausible_arxiv_id(None, "2105.04404"));
    assert!(is_plausible_arxiv_id(None, "0704.0001v2"));
    assert!(is_plausible_arxiv_id(None, "1412.9999"));
    assert!(!is_plausible_arxiv_id(None, "2105.044"));
    assert!(!is_plausible_arxiv_id(None, "21050.4404"));
    assert!(!is_plausible_arxiv_id(None, "math/0211159"));
    // legacy scheme
    assert!(is_plausible_arxiv_id(Some("math"), "0211159"));
    assert!(is_plausible_arxiv_id(Some("math.GT"), "0309136"));
    assert!(is_plausible_arxiv_id(Some("astro-ph"), "9912345v1"));
    assert!(!is_plausible_arxiv_id(Some("math"), "2105.04404"));
    assert!(!is_plausible_arxiv_id(Some("spamarchive"), "0211159"));
    assert!(!is_plausible_arxiv_id(Some("math.G$"), "0309136"));
    assert!(!is_plausible_arxiv_id(Some("math"), "0813159")); // year 08 is new-scheme
  }

  #[test]
  fn unknown_asset_serves_missing_image_fallback() {
    let client = client();
    let response = client
      .get("/html/9999.99999/assets/some/nested/figure.png")
      .dispatch();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(
      response.headers().get_one("Content-Type"),
      Some("image/png")
    );
  }

  #[test]
  fn site_assets_are_served_immutable() {
    let client = client();
    let response = client.get("/assets/ar5iv.0.8.4.css").dispatch();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(
      response.headers().get_one("Cache-Control"),
      Some("public, max-age=31536000, immutable")
    );
  }

  #[test]
  fn robots_txt_is_served() {
    let client = client();
    let response = client.get("/robots.txt").dispatch();
    assert_eq!(response.status(), Status::Ok);
    assert!(response.into_string().unwrap().contains("Disallow: /log/"));
  }
}
