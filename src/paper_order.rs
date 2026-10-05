use regex::Regex;
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::Path;
use std::sync::LazyLock;
use std::{env, fs, io};
use walkdir::WalkDir;

pub static AR5IV_PAPERS_ROOT_DIR: LazyLock<String> = LazyLock::new(|| {
  env::var("AR5IV_PAPERS_ROOT_DIR").unwrap_or_else(|_| String::from("/data/arxmliv"))
});
pub static FIELD_BOUNDARY: LazyLock<Regex> = LazyLock::new(|| Regex::new("([a-z])(\\d)").unwrap());

/// Full rebuild of the `key` hash (`id -> "prev;next"`, wrapping around) from every paper under
/// `root`, walked in sorted order. Already-cached ids are a prefix of the walk and are skipped
/// without writing; a cold cache writes everything. Walks the whole tree: ~20 min on the VM.
pub fn rebuild(conn: &mut redis::Connection, key: &str, root: &str) -> redis::RedisResult<()> {
  // NOTE: HKEYS is O(N) and briefly blocks Redis — fine inside the monthly maintenance window.
  let cached: HashSet<String> = redis::cmd("HKEYS").arg(key).query(conn)?;
  let mut writing = cached.is_empty();

  let mut prev_prev = String::new();
  let mut prev = String::new();
  let mut first = String::new();
  let mut second = String::new();
  let mut buffer = Vec::new();
  let walker = WalkDir::new(root)
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
            // The first not-yet-cached paper marks the boundary: start writing
            // here. `prev` (the last cached paper) is rewritten too as the first
            // push below — its `next` now points to this freshly-added id.
            if !writing && !cached.contains(id.as_ref()) {
              writing = true;
            }
            if writing {
              buffer.push((prev.to_string(), format!("{prev_prev};{id}")));
            }
          }
          prev_prev = prev;
          prev = id.to_string();
        }
      }
    }
    if buffer.len() > 100 {
      save(conn, key, &std::mem::take(&mut buffer))?;
    }
  }

  buffer.push((first.to_string(), format!("{prev};{second}")));
  buffer.push((prev, format!("{prev_prev};{first}")));
  save(conn, key, &buffer)
}

/// Links one month into an existing `key` hash, reading only that month and its nearest non-empty
/// neighbour months (wrapping around, in the same name order as [`rebuild`]): writes the month's
/// own `prev;next` entries, then repoints the previous month's last paper and the next month's
/// first paper at it. Idempotent, so a re-run (or a re-uploaded month) is safe. Returns the number
/// of papers linked.
pub fn link_month(
  conn: &mut redis::Connection,
  key: &str,
  root: &str,
  month: &str,
) -> redis::RedisResult<usize> {
  let root = Path::new(root);
  let mut months: Vec<OsString> = fs::read_dir(root)?
    .filter_map(Result::ok)
    .filter(|entry| entry.path().is_dir())
    .map(|entry| entry.file_name())
    .collect();
  months.sort();
  let pos = months
    .iter()
    .position(|name| name == month)
    .ok_or_else(|| io::Error::other(format!("no month directory {month} under {root:?}")))?;
  let ids = paper_ids(&root.join(&months[pos]));
  let (Some(first), Some(last)) = (ids.first(), ids.last()) else {
    return Ok(0);
  };
  let n = months.len();
  let before = (1..n).find_map(|k| paper_ids(&root.join(&months[(pos + n - k) % n])).pop());
  let after = (1..n).find_map(|k| {
    paper_ids(&root.join(&months[(pos + k) % n]))
      .into_iter()
      .next()
  });

  // The month's own entries; with no other papers anywhere it wraps onto itself.
  let prev_of_first = before.clone().unwrap_or_else(|| last.clone());
  let next_of_last = after.clone().unwrap_or_else(|| first.clone());
  let mut buffer = Vec::with_capacity(ids.len() + 2);
  for (i, id) in ids.iter().enumerate() {
    let prev = if i == 0 { &prev_of_first } else { &ids[i - 1] };
    let next = ids.get(i + 1).unwrap_or(&next_of_last);
    buffer.push((id.clone(), format!("{prev};{next}")));
  }
  // Then the two neighbours, written last so an interrupted run never points into a half-written
  // month.
  if let (Some(before), Some(after)) = (before, after) {
    if before == after {
      buffer.push((before, format!("{last};{first}")));
    } else {
      let (before_prev, _) = cached_pair(conn, key, &before)?;
      let (_, after_next) = cached_pair(conn, key, &after)?;
      buffer.push((before, format!("{before_prev};{first}")));
      buffer.push((after, format!("{last};{after_next}")));
    }
  }
  for chunk in buffer.chunks(1000) {
    save(conn, key, chunk)?;
  }
  Ok(ids.len())
}

/// The papers of one month directory, in [`rebuild`]'s walk order and with its filter.
fn paper_ids(month_dir: &Path) -> Vec<String> {
  let Ok(entries) = fs::read_dir(month_dir) else {
    return Vec::new();
  };
  let mut names: Vec<OsString> = entries
    .filter_map(Result::ok)
    .filter(|entry| entry.path().is_dir())
    .map(|entry| entry.file_name())
    .collect();
  names.sort();
  names
    .iter()
    .map(|name| name.to_string_lossy())
    .filter(|id| id.len() > 4 && id != "arxmliv")
    .map(|id| FIELD_BOUNDARY.replace(&id, "$1/$2").into_owned())
    .collect()
}

/// A neighbour's cached `(prev, next)`; missing means the map is incomplete, so refuse rather than
/// guess.
fn cached_pair(
  conn: &mut redis::Connection,
  key: &str,
  id: &str,
) -> redis::RedisResult<(String, String)> {
  let value: Option<String> = redis::cmd("HGET").arg(key).arg(id).query(conn)?;
  value
    .as_deref()
    .and_then(|pair| pair.split_once(';'))
    .map(|(prev, next)| (prev.to_string(), next.to_string()))
    .ok_or_else(|| {
      io::Error::other(format!(
        "{id} is not in {key}; run a full rebuild (no month argument) first"
      ))
      .into()
    })
}

fn save(
  conn: &mut redis::Connection,
  key: &str,
  buffer: &[(String, String)],
) -> redis::RedisResult<()> {
  redis::pipe().hset_multiple(key, buffer).query(conn)
}
