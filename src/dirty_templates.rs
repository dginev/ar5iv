use crate::cache::set_cached_asset;
use regex::{Captures, Regex};
use rocket_db_pools::deadpool_redis::ConnectionWrapper;
use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

lazy_static! {
  static ref END_ARTICLE: Regex = Regex::new("</article>").unwrap();
  static ref END_HEAD: Regex = Regex::new("</head>").unwrap();
  static ref START_PAGE_CONTENT: Regex = Regex::new("<div class=\"ltx_page_content\">").unwrap();
  static ref END_BODY: Regex = Regex::new("</body>").unwrap();
  static ref SRC_ATTR: Regex = Regex::new(" src=\"([^\"]+)").unwrap();
  static ref HEX_JPG: Regex = Regex::new(r"^ffd8ffe0").unwrap();
  static ref HEX_PNG: Regex = Regex::new(r"^89504e47").unwrap();
  static ref HEX_GIF: Regex = Regex::new(r"^47494638").unwrap();
  static ref AR5IV_PAPERS_ROOT_DIR: String =
    env::var("AR5IV_PAPERS_ROOT_DIR").unwrap_or_else(|_| String::from("/data/arxmliv"));
}

pub fn branded_ar5iv_html(
  mut main_content: String,
  conversion_report: String,
  id_arxiv: String,
) -> String {
  // ensure main_content is a string if undefined
  if main_content.is_empty() {
    main_content = String::from(
      r###"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="Content-Type" content="text/html" />
    <meta charset="utf-8" />
    <title> No content available </title>
    <meta name="language" content="English">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body>
    <div class="ltx_page_main">
    <div class="ltx_page_content">
        <article class="ltx_document">
        </article>
    </div>
    </div>
</body>
</html>
"###,
    );
  }

  // before doing any of our re-branded postprocessing, manage the internal links
  // relativize all src attributes to a current paper directory
  main_content = SRC_ATTR
    .replace_all(&main_content, |caps: &Captures| {
      // leave as-is data URL images and remote sources
      if caps[1].starts_with("data:") || caps[1].starts_with("http") {
        String::from(" src=\"") + &caps[1]
      } else {
        String::from(" src=\"./") + &id_arxiv + "/assets/" + &caps[1]
      }
    })
    .to_string();

  // If a conversion log is present, attach it as a trailing section
  if !conversion_report.is_empty() {
    let ar5iv_logos = r###"
<div class="ar5iv-logos">
    <a href="/"><img height="64" src="/assets/ar5iv.png"></a>
    &nbsp;&nbsp;&nbsp;
    <a href="https://arxiv.org/abs/"###
      .to_string()
      + &id_arxiv
      + r###"" class="arxiv-button">View original paper on arXiv</a>
</div>
"###;
    let html_report = ar5iv_logos
      + r###"
<section id="latexml-conversion-report" class="ltx_section ltx_conversion_report">
    <h2 class="ltx_title ltx_title_section">CorTeX Conversion Report</h2>
    <div id="S1.p1" class="ltx_para">
    <p class="ltx_p">
"### + &conversion_report
      .split('\n')
      .collect::<Vec<_>>()
      .join("</p><p class=\"ltx_p\">")
      + r###"
    </p>
    </div>
</section>
</article>
"###;
    main_content = END_ARTICLE.replace(&main_content, html_report).to_string();
  }

  let maybe_mathjax_js = r###"
    <script>
      var canMathML = typeof(MathMLElement) == "function";
      if (!canMathML) {
      var el = document.createElement("script");
      el.src = "https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js";
      document.querySelector("head").appendChild(el); }
    </script>
    </body>"###;

  let arxmliv_css = r###"
<link media="all" rel="stylesheet" href="//cdn.jsdelivr.net/gh/dginev/arxmliv-css@0.4.1/css/arxmliv.css">
</head>"###;

  main_content = END_HEAD.replace(&main_content, arxmliv_css).to_string();
  main_content = END_BODY
    .replace(&main_content, maybe_mathjax_js)
    .to_string();

  main_content
}

pub async fn assemble_paper(
  conn: &mut ConnectionWrapper,
  field_opt: Option<String>,
  id: &str,
) -> String {
  // Option<File>
  // TODO: Can the tokio::fs::File be swapped in here for some benefit? Does the ZIP crate allow for that?
  //       I couldn't easily understand the answer from what I found online.
  if let Some(paper_path) = build_paper_path(field_opt.as_ref(), id) {
    let zipf = File::open(&paper_path).unwrap();
    let reader = BufReader::new(zipf);
    let mut zip = ZipArchive::new(reader).unwrap();

    let mut log = String::new();
    let mut html = String::new();
    let mut assets = Vec::new();
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
      let cache_key = match field_opt {
        Some(ref field) => field.to_string() + id + "/" + &key,
        None => id.to_string() + "/" + &key,
      };
      set_cached_asset(conn, cache_key.as_str(), &val).await.ok();
    }

    // Lastly, build a single coherent HTML page.
    let id_arxiv = if let Some(ref field) = field_opt {
      format!("{}/{}", field, id)
    } else {
      id.to_owned()
    };
    branded_ar5iv_html(html, log, id_arxiv)
  } else {
    format!(
      "paper id {}{} is not available on disk. ",
      field_opt.unwrap_or_default(),
      id
    )
  }
}

pub async fn assemble_paper_asset(
  field_opt: Option<String>,
  id: &str,
  filename: &str,
) -> Option<Vec<u8>> {
  if let Some(paper_path) = build_paper_path(field_opt.as_ref(), id) {
    let zipf = File::open(&paper_path).unwrap();
    let reader = BufReader::new(zipf);
    let mut zip = ZipArchive::new(reader).unwrap();
    if let Ok(mut asset) = zip.by_name(filename) {
      {
        let mut file_contents = Vec::new();
        asset.read_to_end(&mut file_contents).ok();
        println!(
          "-- assembled {} bytes for asset name {}",
          file_contents.len(),
          filename
        );
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
