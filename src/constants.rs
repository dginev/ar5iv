pub static LOG_FILENAME: &str = "cortex.log";
pub static AR5IV_CSS_URL: &str = "/assets/ar5iv.0.8.5.css";
pub static AR5IV_FONTS_CSS_URL: &str = "/assets/ar5iv-fonts.0.8.4.css";
pub static SITE_CSS_URL: &str = "/assets/ar5iv-site.0.2.2.css";

/// The "glowup" ar5iv-css theme (ar5iv-css v0.9.0, glowup branch), now served to
/// every article -- latexml-oxide is the primary bundle corpus-wide. The site
/// stylesheet (`SITE_CSS_URL`) has no glowup counterpart, so it stays shared.
pub static AR5IV_CSS_GLOWUP_URL: &str = "/assets/ar5iv.0.9.0.css";
pub static AR5IV_FONTS_CSS_GLOWUP_URL: &str = "/assets/ar5iv-fonts.0.9.0.css";

/// The `(fonts_css_url, document_css_url)` pair for an article: every paper now
/// uses the glowup theme. Single source of truth for both the article page and
/// its conversion-report page. (The default `AR5IV_CSS_URL` pair is still used by
/// the homepage and 404 templates.)
pub fn document_css_urls(_id_arxiv: &str) -> (&'static str, &'static str) {
  (AR5IV_FONTS_CSS_GLOWUP_URL, AR5IV_CSS_GLOWUP_URL)
}

pub static DOC_NOT_FOUND_TEMPLATE: &str = r###"<!DOCTYPE html>
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
      <footer class="ltx_page_footer"></footer>
    </div>
</body>
</html>
"###;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn every_article_uses_the_glowup_theme() {
    // legacy ids, modern ids, a future month, and edge cases all resolve to the
    // single glowup theme now that latexml-oxide is the corpus-wide bundle.
    for id in [
      "2606.01234",
      "2605.04404",
      "2601.00001",
      "2701.00001",
      "math/0211159",
      "2606extra",
    ] {
      assert_eq!(
        document_css_urls(id),
        (AR5IV_FONTS_CSS_GLOWUP_URL, AR5IV_CSS_GLOWUP_URL),
        "expected glowup for {id}"
      );
    }
  }
}
