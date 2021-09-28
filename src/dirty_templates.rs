use regex::Regex;
use rustc_serialize::base64::{ToBase64, MIME};
use rustc_serialize::hex::ToHex;

lazy_static! {
  static ref END_ARTICLE: Regex = Regex::new("</article>").unwrap();
  static ref END_HEAD: Regex = Regex::new("</head>").unwrap();
  static ref START_PAGE_CONTENT: Regex = Regex::new("<div class=\"ltx_page_content\">").unwrap();
  static ref END_BODY: Regex = Regex::new("</body>").unwrap();
  static ref SRC_ATTR: Regex = Regex::new(" src=\"").unwrap();
  static ref HEX_JPG: Regex = Regex::new(r"^ffd8ffe0").unwrap();
  static ref HEX_PNG: Regex = Regex::new(r"^89504e47").unwrap();
  static ref HEX_GIF: Regex = Regex::new(r"^47494638").unwrap();
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
  let relativized_src = String::from(" src=\"./") + &id_arxiv + "/";
  main_content = SRC_ATTR
    .replace_all(&main_content, relativized_src)
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
      .split("\n")
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

fn image_buffer_to_data_uri(buffer: Vec<u8>) -> String {
  let b64 = buffer.to_base64(MIME);
  let hex = buffer.to_hex();

  format!("data:image/{};base64,{}", get_hex_type(&hex), b64)
}

fn get_hex_type(file: &str) -> &str {
  if HEX_JPG.is_match(file) {
    "jpg"
  } else if HEX_PNG.is_match(file) {
    "png"
  } else if HEX_GIF.is_match(file) {
    "gif"
  } else {
    eprintln!("-- invalid/unrecognized image file!");
    "png"
  }
}
