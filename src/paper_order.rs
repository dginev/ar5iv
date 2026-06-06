use regex::Regex;
use std::env;
use std::sync::LazyLock;

pub static AR5IV_PAPERS_ROOT_DIR: LazyLock<String> = LazyLock::new(|| {
  env::var("AR5IV_PAPERS_ROOT_DIR").unwrap_or_else(|_| String::from("/data/arxmliv"))
});
pub static FIELD_BOUNDARY: LazyLock<Regex> =
  LazyLock::new(|| Regex::new("([a-z])(\\d)").unwrap());
