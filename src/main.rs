#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
// use rocket::data::{Data, ToByteUnit};
// use rocket::http::uri::Absolute;
// use rocket::response::content::RawText;
use rocket::config::Config;
use rocket::http::Status;
use rocket::response::{content, status};
use rocket::fs::NamedFile;
use rocket::{Build, Request, Rocket};
use rocket_dyn_templates::{context, Metadata, Template};

#[macro_use]
extern crate lazy_static;
use regex::Regex;
use std::fs::File;
use std::collections::HashMap;
use std::env;
use std::io::{Read,BufReader};
use std::path::Path;
use zip::ZipArchive;
use std::borrow::Cow;

lazy_static! {
  static ref TRAILING_PDF_EXT: Regex = Regex::new("[.]pdf$").unwrap();
  static ref END_ARTICLE : Regex = Regex::new("</article>").unwrap();
  static ref END_HEAD : Regex = Regex::new("</head>").unwrap();
  static ref START_PAGE_CONTENT : Regex = Regex::new("<div class=\"ltx_page_content\">").unwrap();
  static ref END_BODY : Regex = Regex::new("</body>").unwrap();
  static ref AR5IV_PAPERS_ROOT_DIR: String =
    env::var("AR5IV_PAPERS_ROOT_DIR").unwrap_or_else(|_| String::from("/data/arxmliv"));
}

#[get("/")]
fn about() -> Template {
  let map: HashMap<String, String> = HashMap::new();
  Template::render("ar5iv", &map)
}

#[get("/abs/<field>/<id>")]
async fn abs_field(field: String, id: String) -> content::RawHtml<String> {
  assemble_paper(Some(field), id).await
}
#[get("/abs/<id>")]
async fn abs(id: String) -> content::RawHtml<String> {
  assemble_paper(None, id).await
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
  let id_base = &id[0..4];
  let id_arxiv = if let Some(ref field) = field_opt {
      format!("{}/{}", field, id)
    } else {
        id.clone()
    };
  let field = field_opt.unwrap_or_default();
  let paper_path_str = format!(
    "{}/{}/{}{}/tex_to_html.zip",
    *AR5IV_PAPERS_ROOT_DIR, id_base, field, id
  );
  let paper_path = Path::new(&paper_path_str);
  if paper_path.exists() {
    // TODO: Can the tokio::fs::File be swapped in here for some benefit? Does the ZIP crate allow for that?
    //       I couldn't easily understand the answer from what I found online.
    let zipf = File::open(&paper_path).unwrap();
    let reader = BufReader::new(zipf);
    let mut zip = ZipArchive::new(reader).unwrap();

    let mut log = String::new();
    let mut html = String::new();
    let mut doc_assets = HashMap::new();
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
                asset = Some(other.to_string());
            } // record assets for later management4
          }
          if let Some(asset_name) = asset {
            let mut file_contents = Vec::new();
            file.read_to_end(&mut file_contents).unwrap();
            doc_assets.insert(asset_name, file_contents);
          }
        }
      }
    }
    
    content::RawHtml(prepare_ar5iv_document(html, log, id_arxiv, doc_assets))
  } else {
    content::RawHtml(format!("paper id {}{} is not available on disk. ", field, id))
  }
}

#[launch]
fn rocket() -> _ {
  rocket::custom(Config::figment().merge(("template_dir", "templates")))
    .attach(Template::fairing())
    .mount("/", routes![abs, abs_field, pdf, pdf_field, about, assets])
    .register("/", catchers![general_not_found, default_catcher])
}



fn prepare_ar5iv_document(mut main_content: String, conversion_report: String, id_arxiv: String, data_url_map: HashMap<String, Vec<u8>>) -> String {
    // ensure main_content is a string if undefined
    if main_content.is_empty() {
      main_content = String::from(r###"
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
"###); }

    // TODO: Add all assets as data URLs.
    for (filename, newurl) in data_url_map {
    //   let escaped_name = 'src=[\'"]' + filename.replace(/([.*+?^=!:${}()|\[\]\/\\])/g, "\\$1") + '[\'"]';
    //   new_src = "src=\"" + newurl + "\"";
    //   main_content = main_content.replace(new RegExp(escaped_name, 'g'), new_src);
    };

    // If a conversion log is present, attach it as a trailing section
    if !conversion_report.is_empty() {
        let ar5iv_logos = r###"
<div class="ar5iv-logos">
    <a href="/"><img height="64" src="/assets/ar5iv.png"></a>
    &nbsp;&nbsp;&nbsp;
    <a href="https://arxiv.org/abs/"###.to_string() + &id_arxiv + r###"" class="arxiv-button">View original paper on arXiv</a>
</div>
"###;
      let html_report = ar5iv_logos + r###"
<section id="latexml-conversion-report" class="ltx_section ltx_conversion_report">
    <h2 class="ltx_title ltx_title_section">CorTeX Conversion Report</h2>
    <div id="S1.p1" class="ltx_para">
    <p class="ltx_p">
"### +
        &conversion_report.split("\n").collect::<Vec<_>>().join("</p><p class=\"ltx_p\">")
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
    main_content = END_BODY.replace(&main_content, maybe_mathjax_js).to_string();
        
    main_content.to_string()
  }