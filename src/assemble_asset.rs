use rocket::http::ContentType;
use rocket::tokio::task::spawn_blocking;
use rocket_db_pools::Connection;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

use crate::cache::{build_arxiv_id, hget_cached, set_cached, set_cached_asset, Cache, TEN_MIB, TWO_AND_A_HALF_MIB};
use crate::constants::LOG_FILENAME;
use crate::dirty_templates::{dirty_branded_ar5iv_html, log_to_html};
use crate::dom_templates::branded_ar5iv_html;
use crate::paper_order::AR5IV_PAPERS_ROOT_DIR;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub enum LatexmlStatus {
  Ok,
  Warning,
  Error,
  Fatal,
}
impl LatexmlStatus {
  pub fn as_css_class(&self) -> &'static str {
    match self {
      LatexmlStatus::Ok => "ar5iv-severity-ok",
      LatexmlStatus::Warning => "ar5iv-severity-warning",
      LatexmlStatus::Error => "ar5iv-severity-error",
      LatexmlStatus::Fatal => "ar5iv-severity-fatal",
    }
  }
}

pub async fn assemble_paper(
  mut conn_opt: Option<Connection<Cache>>,
  field_opt: Option<&str>,
  id: &str,
  use_dom: bool,
) -> Option<String> {
  // Option<File>
  // TODO: Can the tokio::fs::File be swapped in here for some benefit? Does the ZIP crate allow for that?
  //       I couldn't easily understand the answer from what I found online.
  if let Some(paper_path) = build_paper_path(field_opt, id) {
    let id_arxiv = build_arxiv_id(&field_opt, id);
    if let Ok(Ok(mut zip)) = spawn_blocking(move || {
      let zipf = File::open(paper_path)?;
      let reader = BufReader::new(zipf);
      ZipArchive::new(reader)
    })
    .await
    {
      let mut log = String::new();
      let mut html = String::new();
      let mut status = LatexmlStatus::Fatal;
      let mut assets = Vec::new();
      for i in 0..zip.len() {
        if let Ok(mut file) = zip.by_index(i) {
          if file.is_file() {
            let mut asset = None;
            match file.name() {
              name if name.ends_with(".html") => {
                file.read_to_string(&mut html).unwrap();
              }
              name if name == LOG_FILENAME => {
                file.read_to_string(&mut log).unwrap();
              }
              other => {
                // record assets for later management4
                asset = Some(other.to_string());
              }
            }
            if let Some(asset_name) = asset {
              let mut file_contents = Vec::new();
              file.read_to_end(&mut file_contents).unwrap();
              //       the assets should be immediately inserted as we read the ZIP
              //       for async fetching by the browser in the /images/ routes
              if !file_contents.is_empty() {
                assets.push((asset_name, file_contents));
              }
            }
          }
        }
      }
      // if we found assets, cache them.
      for (key, val) in assets.into_iter() {
        if val.len() <= TEN_MIB { // cap using the cache at 10 MiB
          let cache_key = format!("{}/{}", id_arxiv, &key);
          if let Some(ref mut conn) = conn_opt {
            set_cached_asset(conn, cache_key.as_str(), &val).await.ok();
          }
        }
      }
      // the log is dealt with under the /log/ route
      // but since we have it here, cache it
      // (cap cache items at 10 MiB, where a char is 4 bytes)
      if !log.is_empty() && log.len() <= TWO_AND_A_HALF_MIB {
        status = log_to_status(&log);
        let cache_key = format!("{}/{}", id_arxiv, LOG_FILENAME);
        if let Some(ref mut conn) = conn_opt {
          let html_log = log_to_html(&log, &id_arxiv);
          set_cached(conn, &cache_key, &html_log)
            .await
            .ok();
        }
      }
      let mut pieces: Vec<String> = if let Some(ref mut conn) = conn_opt {
        if let Ok(adjacent_papers) = hget_cached(conn, "paper_order", &id_arxiv).await {
          adjacent_papers.split(';').map(|x| x.to_string()).collect()
        } else {
          Vec::new()
        }
      } else {
        Vec::new()
      };
      let next = if pieces.len() < 2 {
        None
      } else {
        Some(pieces.pop().unwrap())
      };
      let prev = if pieces.is_empty() {
        None
      } else {
        Some(pieces.pop().unwrap())
      };
      // Lastly, build a single coherent HTML page.
      let branded_html = if use_dom {
        branded_ar5iv_html(html, &id_arxiv, status, prev, next)
      } else {
        dirty_branded_ar5iv_html(html, &id_arxiv, status, prev, next)
      };
      if branded_html.len() < TWO_AND_A_HALF_MIB { // cap cache items at 10 MiB, where a char is 4 bytes
        if let Some(ref mut conn) = conn_opt {
          set_cached(&mut *conn, &id_arxiv, branded_html.as_str())
            .await
            .ok();
        }
      }
      Some(branded_html)
    } else {
      None
    }
  } else {
    None
  }
}

pub async fn assemble_paper_asset(
  field_opt: Option<&str>,
  id: &str,
  filename: &str,
) -> Option<Vec<u8>> {
  if let Some(paper_path) = build_paper_path(field_opt, id) {
    if let Ok(mut zip) = spawn_blocking(move || {
      let zipf = File::open(paper_path).unwrap();
      let reader = BufReader::new(zipf);
      ZipArchive::new(reader).unwrap()
    })
    .await
    {
      if let Ok(mut asset) = zip.by_name(filename) {
        let mut file_contents = Vec::new();
        asset.read_to_end(&mut file_contents).ok();
        Some(file_contents)
      } else {
        None
      }
    } else {
      None
    }
  } else {
    None
  }
}

pub fn fetch_zip(field_opt: Option<&str>, id: &str) -> Option<(ContentType, Vec<u8>)> {
  if let Some(paper_path) = build_source_zip_path(field_opt, id) {
    let zipf = File::open(paper_path).unwrap();
    let mut reader = BufReader::new(zipf);
    let mut payload = Vec::new();
    reader.read_to_end(&mut payload).ok();
    if payload.is_empty() {
      None
    } else {
      Some((ContentType::ZIP, payload))
    }
  } else {
    None
  }
}

pub async fn assemble_log(field_opt: Option<&str>, id: &str) -> Option<String> {
  if let Some(paper_path) = build_paper_path(field_opt, id) {
    if let Ok(mut zip) = spawn_blocking(move || {
      let zipf = File::open(paper_path).unwrap();
      let reader = BufReader::new(zipf);
      ZipArchive::new(reader).unwrap()
    })
    .await
    {
      if let Ok(mut asset) = zip.by_name(LOG_FILENAME) {
        let mut conversion_report: String = String::new();
        asset.read_to_string(&mut conversion_report).ok();
        let id_arxiv = if let Some(ref field) = field_opt {
          format!("{}/{}", field, id)
        } else {
          id.to_owned()
        };
        Some(log_to_html(&conversion_report, &id_arxiv))
      } else {
        None
      }
    } else {
      None
    }
  } else {
    None
  }
}

fn log_to_status(log: &str) -> LatexmlStatus {
  let mut status = LatexmlStatus::Ok;
  for line in log.lines() {
    if line.starts_with("Warning:") && status < LatexmlStatus::Warning {
      status = LatexmlStatus::Warning;
    } else if line.starts_with("Error:") && status < LatexmlStatus::Error {
      status = LatexmlStatus::Error;
    } else if line.starts_with("Fatal:") && status < LatexmlStatus::Fatal {
      status = LatexmlStatus::Fatal;
    } else if line.starts_with("Status:conversion:") {
      match line.chars().nth(18).unwrap() {
        '0' => {
          if status <= LatexmlStatus::Ok {
            status = LatexmlStatus::Ok
          }
        }
        '1' => {
          if status < LatexmlStatus::Warning {
            status = LatexmlStatus::Warning
          }
        }
        '2' => {
          if status < LatexmlStatus::Error {
            status = LatexmlStatus::Error
          }
        }
        _ => status = LatexmlStatus::Fatal,
      }
    }
  }
  status
}

fn build_paper_path(field_opt: Option<&str>, id: &str) -> Option<PathBuf> {
  // basic sanity: valid ids are 4+ characters.
  if id.len() < 4 {
    return None;
  }
  let id_base = &id[0..4];
  let paper_path_str = format!(
    "{}/{}/{}{}/tex_to_html.zip",
    *AR5IV_PAPERS_ROOT_DIR,
    id_base,
    field_opt.unwrap_or(""),
    id
  );
  let paper_path = Path::new(&paper_path_str);
  if paper_path.exists() {
    Some(paper_path.to_path_buf())
  } else {
    None
  }
}

fn build_source_zip_path(field_opt: Option<&str>, id: &str) -> Option<PathBuf> {
  let id_base = &id[0..4];
  let field = field_opt.unwrap_or("");
  let paper_path_str = format!(
    "{}/{}/{}{}/{}{}.zip",
    *AR5IV_PAPERS_ROOT_DIR, id_base, field, id, field, id
  );
  let paper_path = Path::new(&paper_path_str);
  if paper_path.exists() {
    Some(paper_path.to_path_buf())
  } else {
    None
  }
}
