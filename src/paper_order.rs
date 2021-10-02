use regex::Regex;
use std::env;

lazy_static! {
  pub static ref AR5IV_PAPERS_ROOT_DIR: String =
    env::var("AR5IV_PAPERS_ROOT_DIR").unwrap_or_else(|_| String::from("/data/arxmliv"));
  pub static ref FIELD_BOUNDARY: Regex = Regex::new("([a-z])(\\d)").unwrap();
}
