use crate::assemble_asset::LatexmlStatus;
use crate::constants::{AR5IV_CSS_URL,SITE_CSS_URL,DOC_NOT_FOUND_TEMPLATE};
use regex::{Captures, Regex};

lazy_static! {
  static ref END_ARTICLE: Regex = Regex::new("</article>").unwrap();
  static ref END_HEAD: Regex = Regex::new("</head>").unwrap();
  static ref END_BODY: Regex = Regex::new("</body>").unwrap();
  static ref START_PAGE_CONTENT: Regex = Regex::new("<div class=\"ltx_page_content\">").unwrap();
  static ref START_FOOTER: Regex = Regex::new("<footer class=\"ltx_page_footer\">").unwrap();
  static ref TITLE_ELEMENT: Regex = Regex::new("<title>([^<]+)</title>").unwrap();
  static ref SRC_ATTR: Regex = Regex::new(" src=\"([^\"]+)").unwrap();
  static ref DATA_SVG_ATTR: Regex = Regex::new(" data=\"([^\"]+)[.]svg").unwrap();
  static ref HEX_JPG: Regex = Regex::new(r"^ffd8ffe0").unwrap();
  static ref HEX_PNG: Regex = Regex::new(r"^89504e47").unwrap();
  static ref HEX_GIF: Regex = Regex::new(r"^47494638").unwrap();
  static ref EXTERNAL_HREF: Regex = Regex::new(" href=\"http").unwrap();
}

pub fn dirty_branded_ar5iv_html(
  mut main_content: String,
  id_arxiv: &str,
  status: LatexmlStatus,
  prev: Option<String>,
  next: Option<String>,
) -> String {
  let status_css_class = status.as_css_class();
  // ensure main_content is a string if undefined
  if main_content.is_empty() {
    main_content = DOC_NOT_FOUND_TEMPLATE.to_string();
  } else {
    // ensure we have a lang attribute otherwise, English being most common in arXiv
    main_content = main_content.replacen("<html>", "<html lang=\"en\">", 1);
    
    // also add the arxiv id to the title element
    //
    // Note: replacen would be faster, but we can't access the title content
    // .replacen("<title>", &format!("<title>[{}] ",id_arxiv), 1);
    
    // This is also the best place to insert vendor-specific meta tags
    main_content = TITLE_ELEMENT.replace(&main_content, |caps: &Captures| {
      String::from("<title>[")+id_arxiv+"] "+&caps[1]+"</title>"+r###"
<meta name="twitter:card" content="summary">
<meta name="twitter:title" content=""###+&caps[1]+r###"">
<meta name="twitter:image:src" content="https://ar5iv.org/assets/ar5iv_card.png">
<meta name="twitter:image:alt" content="ar5iv logo">
<meta property="og:title" content=""###+&caps[1]+r###"">
<meta property="og:site_name" content="ar5iv">
<meta property="og:image" content="https://ar5iv.org/assets/ar5iv_card.png">
<meta property="og:type" content="article">
<meta property="og:url" content="https://ar5iv.org/html/"###+id_arxiv+r###"">
"### }).to_string();
  }

  let main_content_src = SRC_ATTR.replace_all(&main_content, |caps: &Captures| {
    // leave as-is data URL images and remote sources
    if caps[1].starts_with("data:") || caps[1].starts_with("http") {
      String::from(" src=\"") + &caps[1]
    } else {
      // there is a catch here in the ar5iv.org setting.
      // the old ID scheme has an extra component in the relative path, e.g. compare
      // ./astro-ph/0001016
      // to the modern
      // ./2105.04404
      // so we should *always* use the id *without* the field,
      // when pointing from within a document to an asset under it.
      //
      // NEW: Rather than struggle with relativistic issues, let's just do the absolute path.
      String::from(" src=\"/html/") + id_arxiv + "/assets/" + &caps[1]
    }
  });
  main_content = DATA_SVG_ATTR
    .replace_all(&main_content_src, |caps: &Captures| {
      if caps[1].starts_with("data:") || caps[1].starts_with("http") {
        String::from(" data=\"") + &caps[1] + ".svg"
      } else {
        String::from(" data=\"/html/") + id_arxiv + "/assets/" + &caps[1] + ".svg"
      }
    })
    .to_string();
  main_content = EXTERNAL_HREF.replace_all(&main_content," target=\"_blank\" href=\"http").to_string();
  // if this is a Fatal conversion, warn readers explicitly.
  let status_message = if status == LatexmlStatus::Fatal {
    r###"
<div class="ltx_document"><div class="ltx_para"><div class="ltx_p"><span class="ltx_ERROR">
Conversion to HTML had a Fatal error and exited abruptly. This document may be truncated or damaged.
</span></div></div></div>
</article>
"###
      .to_string()
  } else {
    String::new()
  };

  // If a conversion log is present, attach it as a trailing section
  let prev_html = if let Some(prev_id) = prev {
    format!(
      "<a href=\"/html/{}\" class=\"ar5iv-nav-button ar5iv-nav-button-prev\">◄</a>",
      prev_id
    )
  } else {
    String::from(
      "<a href=\"javascript: void(0)\" class=\"ar5iv-nav-button ar5iv-nav-button-prev\">◄</a>",
    )
  };
  let next_html = if let Some(next_id) = next {
    format!(
      "<a href=\"/html/{}\" class=\"ar5iv-nav-button ar5iv-nav-button-next\">►</a>",
      next_id
    )
  } else {
    String::from(
      "<a href=\"javascript: void(0)\" class=\"ar5iv-nav-button ar5iv-nav-button-next\">►</a>",
    )
  };
    // Hide for now: tex source button
    // <a class="ar5iv-text-button" href="/source/"###
    //+ id_arxiv
    //+ r###".zip" class="ar5iv-text-button">Download<br>TeX&nbsp;source</a>
  let ar5iv_footer = status_message
    + "<div class=\"ar5iv-footer\">"
    + &prev_html
    + r###"
    <a class="ar5iv-home-button" href="/"><img height="40" alt="ar5iv homepage" src="/assets/ar5iv.png"></a>
    <a href="/feeling_lucky" class="ar5iv-text-button">Feeling<br>lucky?</a>
    <a href="/log/"###
    + id_arxiv
    + r###"" class="ar5iv-text-button "###
    + status_css_class
    + r###"">Conversion<br>report</a>
    <a href="https://arxiv.org/abs/"###
    + id_arxiv
    + r###"" class="ar5iv-text-button arxiv-ui-theme">View&nbsp;original<br>on&nbsp;arXiv</a>"###
    + &next_html
    + r###"
</div><footer class="ltx_page_footer">
<a class="ar5iv-toggle-color-scheme" href="javascript:toggleColorScheme()" title="Toggle ar5iv color scheme"><span class="color-scheme-icon"></span></a>
"###;
  main_content = START_FOOTER
    .replace(&main_content, ar5iv_footer)
    .to_string();
  // Hide the polyfill dirty work behind a curtain
  let active_js = concat!(
    r###"
    <script>
      var canMathML = typeof(MathMLElement) == "function";
      if (!canMathML) {
        var body = document.querySelector("body");
        body.firstElementChild.setAttribute('style', 'opacity: 0;');
        var loading = document.createElement("div");
        loading.setAttribute("id", "mathjax-loading-spinner");
        var message = document.createElement("div");
        message.setAttribute("id", "mathjax-loading-message");
        message.innerText = "Typesetting Equations...";
        body.prepend(loading);
        body.prepend(message);

        var el = document.createElement("script");
        el.src = "https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js";
        document.querySelector("head").appendChild(el);

        window.MathJax = {
          startup: {
            pageReady: () => {
              return MathJax.startup.defaultPageReady().then(() => {
                body.removeChild(loading);
                body.removeChild(message);
                body.firstElementChild.removeAttribute('style');
              }); } } };
      }      
    </script>"###,
    // Thanks to https://stackoverflow.com/questions/56300132/how-to-override-css-prefers-color-scheme-setting
    // local storage is used to override OS theme settings
    r###"
    <script>
      function detectColorScheme(){
        var theme="light";
        var current_theme = localStorage.getItem("ar5iv_theme");
        if(current_theme){
          if(current_theme == "dark"){
            theme = "dark";
          } }
        else if(!window.matchMedia) { return false; }
        else if(window.matchMedia("(prefers-color-scheme: dark)").matches) {
          theme = "dark"; }
        if (theme=="dark") {
          document.documentElement.setAttribute("data-theme", "dark");
        } else {
          document.documentElement.setAttribute("data-theme", "light"); } }
      
      detectColorScheme();
      
      function toggleColorScheme(){
        var current_theme = localStorage.getItem("ar5iv_theme");
        if (current_theme) {
          if (current_theme == "light") {
            localStorage.setItem("ar5iv_theme", "dark"); }
          else {
            localStorage.setItem("ar5iv_theme", "light"); } }
        else {
            localStorage.setItem("ar5iv_theme", "dark"); }
        detectColorScheme(); }
    </script>"###,
    // Let's experiment with an inline bibitem preview 
    r###"
    <script>
    // Auxiliary function, building the preview feature when
    // an inline citation is clicked
    function clicked_cite(e) {
      e.preventDefault();
      let cite = this.closest('.ltx_cite');
      let next = cite.nextSibling;
      if (next && next.nodeType == Node.ELEMENT_NODE && next.getAttribute('class') == "ar5iv-bibitem-preview") {
        next.remove();
        return; }
      // Before adding a preview modal,
      // cleanup older previews, in case they're still open
      document.querySelectorAll('span.ar5iv-bibitem-preview').forEach(function(node) {
        node.remove();
      })
      
      // Create the preview
      preview = document.createElement('span');
      preview.setAttribute('class','ar5iv-bibitem-preview');
      let target = document.getElementById(this.getAttribute('href').slice(1));
      target.childNodes.forEach(function (child) {
        preview.append(child.cloneNode(true));
      });
      let close_x = document.createElement('button');
      close_x.setAttribute("aria-label","Close modal for bibliography item preview");
      close_x.textContent = "×";
      close_x.setAttribute('class', 'ar5iv-button-close-preview');
      close_x.setAttribute('onclick','this.parentNode.remove()');
      preview.append(close_x);
      preview.querySelectorAll('.ltx_tag_bibitem').forEach(function(node) {
        node.remove();
      });
      cite.parentNode.insertBefore(preview, cite.nextSibling);
      return;
    }
    // Global Document initialization:
    // - assign the preview feature to all inline citation links
    document.querySelectorAll(".ltx_cite .ltx_ref").forEach(function (link) {
      link.addEventListener("click", clicked_cite);
    });
    </script>
    "###,
    "</body>"
  );
  let css = String::from("<link media=\"all\" rel=\"stylesheet\" href=\"")
    + AR5IV_CSS_URL
    + "\"><link media=\"all\" rel=\"stylesheet\" href=\""
    + SITE_CSS_URL
    + "\">
</head>";

  main_content = END_HEAD.replace(&main_content, css).to_string();
  main_content = END_BODY.replace(&main_content, active_js).to_string();
  main_content
}


pub fn log_to_html(conversion_report: &str, id_arxiv: &str) -> String {
  String::from(
    r###"<!DOCTYPE html><html>
<head>
<title>Conversion report for arXiv article "###,
  ) + id_arxiv
    + r###"</title>
<meta http-equiv="Content-Type" content="text/html; charset=UTF-8">
<meta name="robots" content="noindex">
<link media="all" rel="stylesheet" href=""###
    + AR5IV_CSS_URL
    + r###"">
</head>
<body>
<div class="ltx_page_main">
<div class="ltx_page_content">
<article class="ltx_document ltx_authors_1line">
  <section id="latexml-conversion-report" class="ltx_section ltx_conversion_report">
    <h2 class="ltx_title ltx_title_section">LaTeXML conversion report (<a class="ltx_ref" href="/html/"###
    + id_arxiv
    + "\">"
    + id_arxiv
    + r###"</a>)</h2>
    <div id="S1.p1" class="ltx_para">
      <p class="ltx_p">
"### + &conversion_report
    .split('\n')
    .map(|line| {
      let line = line.replace('\t', "&emsp;");
      if line.starts_with("Warning:") {
        "</p><p class=\"ltx_p\"><span class=\"ltx_WARNING\">".to_string() + &line + "</span>"
      } else if line.starts_with("Error:") {
        "</p><p class=\"ltx_p\"><span class=\"ltx_ERROR\">".to_string() + &line + "</span>"
      } else if line.starts_with("Info:") {
        "</p><p class=\"ltx_p\"><span class=\"ltx_INFO\">".to_string() + &line + "</span>"
      } else if line.starts_with("Fatal:") {
        "</p><p class=\"ltx_p\"><span class=\"ltx_FATAL\">".to_string() + &line + "</span>"
      } else if line.starts_with("Conversion complete:")
        || line.starts_with("Post-processing complete:")
      {
        // provide a colored final status
        if line.contains(" fatal") {
          "</p><p class=\"ltx_p\"><span class=\"ltx_FATAL\">".to_string() + &line + "</span>"
        } else if line.contains(" error") {
          "</p><p class=\"ltx_p\"><span class=\"ltx_ERROR\">".to_string() + &line + "</span>"
        } else if line.contains(" warning") {
          "</p><p class=\"ltx_p\"><span class=\"ltx_WARNING\">".to_string() + &line + "</span>"
        } else {
          "</p><p class=\"ltx_p\"><span class=\"ltx_INFO\">".to_string() + &line + "</span>"
        }
      } else {
        line
      }
    })
    .collect::<Vec<_>>()
    .join("<br>\n")
    + r###"
      </p>
    </div>
  </section>
</article>
</div></div>
</body>
</html>"###
}
