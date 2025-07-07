use ar5iv::paper_order::{AR5IV_PAPERS_ROOT_DIR, FIELD_BOUNDARY};
use walkdir::WalkDir;

fn main() -> redis::RedisResult<()> {
  let client = redis::Client::open("redis://127.0.0.1/")?;
  let mut conn = client.get_connection()?;
  // This isn't really needed as deletions are disruptive on production machines
  // redis::cmd("DEL").arg("paper_order").query(&mut conn)?;

  let mut prev_prev = String::new();
  let mut prev = String::new();
  let mut first = String::new();
  let mut second = String::new();
  let mut buffer = Vec::new();
  let walker = WalkDir::new(AR5IV_PAPERS_ROOT_DIR.to_string())
    .min_depth(2)
    .max_depth(2)
    .sort_by_file_name()
    .follow_links(true);
  for entry_result in walker {
    if let Ok(entry) = entry_result {
      let entry_path = entry.path();
      if entry_path.is_dir() {
        let id_like = entry_path.file_name().unwrap_or_default().to_string_lossy();
        if id_like.len() > 4 && id_like != "arxmliv" {
          let id = FIELD_BOUNDARY.replace(&id_like, "$1/$2");
          if prev_prev.is_empty() && !prev.is_empty() && first.is_empty() {
            first = prev.to_string();
            second = id.to_string();
          } else if !prev_prev.is_empty() {
            buffer.push((prev.to_string(), format!("{prev_prev};{id}")));
          }
          prev_prev = prev;
          prev = id.to_string();
        }
      }
    }
    if buffer.len() > 100 {
      save_to_cache(&mut conn, std::mem::take(&mut buffer))?;
    }
  }

  buffer.push((first.to_string(), format!("{prev};{second}")));
  buffer.push((prev, format!("{prev_prev};{first}")));
  save_to_cache(&mut conn, buffer)
}

fn save_to_cache(
  conn: &mut redis::Connection,
  buffer: Vec<(String, String)>,
) -> redis::RedisResult<()> {
  redis::pipe()
    .hset_multiple("paper_order", &buffer)
    .query(conn)
}
