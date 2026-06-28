pub static LOG_FILENAME: &str = "cortex.log";
pub static AR5IV_CSS_URL: &str = "/assets/ar5iv.0.8.4.css";
pub static AR5IV_FONTS_CSS_URL: &str = "/assets/ar5iv-fonts.0.8.4.css";
pub static SITE_CSS_URL: &str = "/assets/ar5iv-site.0.2.2.css";

/// The "glowup" ar5iv-css theme (ar5iv-css v0.9.0, glowup branch), served to a
/// rolling set of recent arXiv months instead of the default theme above. The
/// site stylesheet (`SITE_CSS_URL`) has no glowup counterpart, so it stays shared.
pub static AR5IV_CSS_GLOWUP_URL: &str = "/assets/ar5iv.0.9.0.css";
pub static AR5IV_FONTS_CSS_GLOWUP_URL: &str = "/assets/ar5iv-fonts.0.9.0.css";

/// arXiv id prefixes whose articles are served the glowup theme. These are the
/// months first generated with latexml-oxide (2026-06 onward); extend the list
/// as more months are (re)processed. Matched against the version-stripped id
/// (e.g. "2606.01234"); the trailing '.' is part of every modern id, so "2606."
/// pins the match to year 26 / month 06 and never to a legacy id like
/// "math/0211159".
pub static GLOWUP_ID_PREFIXES: &[&str] = &[
  "2606.", "2607.", "2608.", "2609.", "2610.", "2611.", "2612.",
];

/// Whether an article's (version-stripped) arxiv id should use the glowup theme.
pub fn uses_glowup_theme(id_arxiv: &str) -> bool {
  GLOWUP_ID_PREFIXES
    .iter()
    .any(|prefix| id_arxiv.starts_with(prefix))
}

/// The `(fonts_css_url, document_css_url)` pair for an article: the glowup theme
/// for ids in `GLOWUP_ID_PREFIXES`, otherwise the default. Single source of
/// truth for both the article page and its conversion-report page.
pub fn document_css_urls(id_arxiv: &str) -> (&'static str, &'static str) {
  if uses_glowup_theme(id_arxiv) {
    (AR5IV_FONTS_CSS_GLOWUP_URL, AR5IV_CSS_GLOWUP_URL)
  } else {
    (AR5IV_FONTS_CSS_URL, AR5IV_CSS_URL)
  }
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
  fn glowup_months_select_the_glowup_theme() {
    for id in ["2606.01234", "2606.1234", "2609.00001", "2612.99999"] {
      assert!(uses_glowup_theme(id), "expected glowup for {id}");
      assert_eq!(
        document_css_urls(id),
        (AR5IV_FONTS_CSS_GLOWUP_URL, AR5IV_CSS_GLOWUP_URL)
      );
    }
  }

  #[test]
  fn other_ids_keep_the_default_theme() {
    // earlier 2026 months, a future month past the rollout, a legacy id, and a
    // would-be prefix collision all stay on the default stylesheet.
    for id in [
      "2605.04404",
      "2601.00001",
      "2701.00001",
      "math/0211159",
      "2606extra",
    ] {
      assert!(!uses_glowup_theme(id), "expected default for {id}");
      assert_eq!(document_css_urls(id), (AR5IV_FONTS_CSS_URL, AR5IV_CSS_URL));
    }
  }
}
