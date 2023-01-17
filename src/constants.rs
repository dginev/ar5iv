pub static LOG_FILENAME: &str = "cortex.log";
pub static AR5IV_CSS_URL: &str = "/assets/ar5iv.0.7.6.min.css";
pub static SITE_CSS_URL: &str = "/assets/ar5iv-site.0.2.1.css";

pub static DOC_NOT_FOUND_TEMPLATE : &str = r###"<!DOCTYPE html>
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
