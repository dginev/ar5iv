use rocket::fs::NamedFile;
use rocket::tokio::task::spawn_blocking;
use rocket_db_pools::Connection;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

use crate::cache::{
  asset_key, build_arxiv_id, hget_cached, log_key, paper_key, set_cached, set_cached_asset, Cache,
  SIXTY_FOUR_MIB, TEN_MIB,
};
use crate::constants::LOG_FILENAME;
use crate::dirty_templates::{dirty_branded_ar5iv_html, log_to_html};
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

/// The pieces of a paper's ZIP that we extract for serving.
struct PaperParts {
  html: String,
  log: String,
  assets: Vec<(String, Vec<u8>)>,
}

pub async fn assemble_paper(
  mut conn_opt: Option<Connection<Cache>>,
  field_opt: Option<&str>,
  id: &str,
) -> Option<String> {
  let paper_path = build_paper_path(field_opt, id)?;
  let id_arxiv = build_arxiv_id(&field_opt, id);
  // Open, scan and decompress the ZIP entirely inside a blocking task:
  // decompression is CPU-bound work that would otherwise stall the async workers.
  // I/O errors (e.g. a ZIP being replaced mid-request by a data update)
  // degrade to None instead of panicking.
  let parts = spawn_blocking(move || -> Option<PaperParts> {
    let zipf = File::open(paper_path).ok()?;
    let reader = BufReader::new(zipf);
    let mut zip = ZipArchive::new(reader).ok()?;
    let mut html = String::new();
    let mut log = String::new();
    let mut assets = Vec::new();
    for i in 0..zip.len() {
      if let Ok(mut file) = zip.by_index(i) {
        if file.is_file() {
          let mut asset = None;
          match file.name() {
            name if name.ends_with(".html") => {
              if html.is_empty() {
                // a damaged main document makes the paper unusable.
                file.read_to_string(&mut html).ok()?;
              }
              // (additional .html entries are ignored)
            }
            name if name == LOG_FILENAME => {
              // a damaged log is survivable.
              if file.read_to_string(&mut log).is_err() {
                log.clear();
              }
            }
            other => {
              // record assets for later caching.
              // skip oversized assets: they can't be cached (see TEN_MIB cap below),
              // so buffering them here only inflates RSS -- the /assets/ routes
              // serve them from the ZIP on demand instead.
              if file.size() <= TEN_MIB as u64 {
                asset = Some(other.to_string());
              }
            }
          }
          if let Some(asset_name) = asset {
            let mut file_contents = Vec::new();
            // a damaged asset is survivable.
            if file.read_to_end(&mut file_contents).is_ok() && !file_contents.is_empty() {
              assets.push((asset_name, file_contents));
            }
          }
        }
      }
    }
    Some(PaperParts { html, log, assets })
  })
  .await
  .ok()
  .flatten()?;
  let PaperParts { html, log, assets } = parts;
  // the log determines the conversion-status badge for the footer.
  let status = if log.is_empty() {
    LatexmlStatus::Fatal
  } else {
    log_to_status(&log)
  };
  // fish out the prev/next paper ids for the footer navigation.
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
  // Build a single coherent HTML page -- also off the async workers,
  // since the regex branding pass is CPU-bound.
  let id_arxiv_branding = id_arxiv.clone();
  let status_branding = status.clone();
  let branded_html = spawn_blocking(move || {
    dirty_branded_ar5iv_html(html, &id_arxiv_branding, status_branding, prev, next)
  })
  .await
  .ok()?;
  // Cache the paper itself, so the next request is a fast Redis hit.
  if branded_html.len() <= TEN_MIB {
    // cap cache items at 10 MiB
    if let Some(ref mut conn) = conn_opt {
      set_cached(&mut *conn, &paper_key(&id_arxiv), branded_html.as_str())
        .await
        .ok();
    }
  }
  // Warm the asset and log caches in a detached task, off this request's
  // critical path -- the browser will start fetching the assets as soon as
  // it receives the HTML we are about to return.
  if let Some(mut conn) = conn_opt {
    rocket::tokio::spawn(async move {
      for (name, val) in assets.into_iter() {
        if val.len() <= TEN_MIB {
          // cap cache items at 10 MiB
          let cache_key = asset_key(&id_arxiv, &name);
          set_cached_asset(&mut conn, cache_key.as_str(), &val).await.ok();
        }
      }
      if !log.is_empty() && log.len() <= TEN_MIB {
        let html_log = log_to_html(&log, &id_arxiv);
        set_cached(&mut conn, &log_key(&id_arxiv), &html_log).await.ok();
      }
    });
  }
  Some(branded_html)
}

pub async fn assemble_paper_asset(
  field_opt: Option<&str>,
  id: &str,
  filename: &str,
) -> Option<Vec<u8>> {
  let paper_path = build_paper_path(field_opt, id)?;
  let filename = filename.to_string();
  spawn_blocking(move || {
    let zipf = File::open(paper_path).ok()?;
    let reader = BufReader::new(zipf);
    let mut zip = ZipArchive::new(reader).ok()?;
    let mut asset = zip.by_name(&filename).ok()?;
    // refuse to buffer pathologically large assets into RAM
    if asset.size() > SIXTY_FOUR_MIB {
      return None;
    }
    let mut file_contents = Vec::with_capacity(asset.size() as usize);
    asset.read_to_end(&mut file_contents).ok()?;
    Some(file_contents)
  })
  .await
  .ok()
  .flatten()
}

pub async fn fetch_zip(field_opt: Option<&str>, id: &str) -> Option<NamedFile> {
  if let Some(paper_path) = build_source_zip_path(field_opt, id) {
    // stream the ZIP from disk instead of buffering it into RAM;
    // NamedFile also derives the application/zip content type from the extension.
    NamedFile::open(paper_path).await.ok()
  } else {
    None
  }
}

pub async fn assemble_log(field_opt: Option<&str>, id: &str) -> Option<String> {
  let paper_path = build_paper_path(field_opt, id)?;
  let id_arxiv = build_arxiv_id(&field_opt, id);
  spawn_blocking(move || {
    let zipf = File::open(paper_path).ok()?;
    let reader = BufReader::new(zipf);
    let mut zip = ZipArchive::new(reader).ok()?;
    let mut asset = zip.by_name(LOG_FILENAME).ok()?;
    let mut conversion_report = String::new();
    asset.read_to_string(&mut conversion_report).ok()?;
    Some(log_to_html(&conversion_report, &id_arxiv))
  })
  .await
  .ok()
  .flatten()
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
      // (a status line truncated right after the prefix counts as fatal)
      match line.chars().nth(18) {
        Some('0') => {
          if status <= LatexmlStatus::Ok {
            status = LatexmlStatus::Ok
          }
        }
        Some('1') => {
          if status < LatexmlStatus::Warning {
            status = LatexmlStatus::Warning
          }
        }
        Some('2') => {
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
  // basic sanity: valid ids start with at least 4 characters (e.g. "YYMM").
  // `get` returns None -- rather than panicking -- for short ids, and for ids
  // where byte 4 would split a multi-byte UTF-8 character.
  let id_base = id.get(0..4)?;
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
  let id_base = id.get(0..4)?;
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn truncated_status_line_is_fatal_not_a_panic() {
    // regression: `line.chars().nth(18).unwrap()` used to panic here
    assert_eq!(log_to_status("Status:conversion:"), LatexmlStatus::Fatal);
  }

  #[test]
  fn status_lines_map_to_severities() {
    assert_eq!(log_to_status("Status:conversion:0"), LatexmlStatus::Ok);
    assert_eq!(log_to_status("Status:conversion:1"), LatexmlStatus::Warning);
    assert_eq!(log_to_status("Status:conversion:2"), LatexmlStatus::Error);
    assert_eq!(log_to_status("Status:conversion:3"), LatexmlStatus::Fatal);
    assert_eq!(
      log_to_status("Warning: x\nStatus:conversion:0"),
      LatexmlStatus::Warning
    );
    assert_eq!(
      log_to_status("Fatal: broken\nStatus:conversion:0"),
      LatexmlStatus::Fatal
    );
  }

  #[test]
  fn short_and_multibyte_ids_build_no_path() {
    // regression: `&id[0..4]` used to panic on both of these
    assert_eq!(build_paper_path(None, "abc"), None);
    assert_eq!(build_source_zip_path(None, "abc"), None);
    assert_eq!(build_paper_path(None, "ab€cd"), None);
    assert_eq!(build_source_zip_path(None, "ab€cd"), None);
  }
}
