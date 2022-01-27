use crate::assemble_asset::LatexmlStatus;
// use crate::constants::{DOC_NOT_FOUND_TEMPLATE};

pub fn branded_ar5iv_html(
  _main_content: String,
  _id_arxiv: &str,
  _status: LatexmlStatus,
  _prev: Option<String>,
  _next: Option<String>,
) -> String {
  // let _status_css_class = status.as_css_class();
  // if main_content.is_empty() {
  //   main_content = DOC_NOT_FOUND_TEMPLATE.to_string();
  // }
  /* TODO: Maybe continue here?
     Are we ready to spend the ~0.5 second for the libxml reserialization?
     The benchmark was looking decisive already before running any of the needed XPaths:
  
  assemble dirty with regex                                                                            
    time:   [101.78 ms 103.31 ms 104.86 ms]

  assemble with dom
    time:   [595.14 ms 610.13 ms 624.77 ms]                              
  */
  unimplemented!();
  
  // let parser = Parser::default_html();
  // let mut dom = parser.parse_string(main_content).unwrap();
  // TODO: ... brand for ar5iv ...
  // Return:
  // dom.to_string_with_options(SaveOptions {
  //   format: false,
  //   no_declaration: true,
  //   no_empty_tags: true,
  //   no_xhtml: true,
  //   xhtml: false,
  //   as_xml: false,
  //   as_html: true,
  //   non_significant_whitespace: true
  // })
}
