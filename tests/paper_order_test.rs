//! The incremental month link must leave exactly the map a full rebuild would. Uses a throwaway tree
//! under `/tmp` and per-process hash keys on the local Redis; skips when no Redis is reachable (CI).
use ar5iv::paper_order::{link_month, rebuild};
use std::collections::HashMap;
use std::fs;

fn papers(root: &str, month: &str, ids: &[&str]) {
  for id in ids {
    fs::create_dir_all(format!("{root}/{month}/{id}")).unwrap();
  }
}

fn map(conn: &mut redis::Connection, key: &str) -> HashMap<String, String> {
  redis::cmd("HGETALL").arg(key).query(conn).unwrap()
}

#[test]
fn linking_a_new_month_matches_a_full_rebuild() {
  let Ok(mut conn) = redis::Client::open("redis://127.0.0.1/").and_then(|c| c.get_connection())
  else {
    eprintln!("skipping: no Redis at 127.0.0.1");
    return;
  };
  let pid = std::process::id();
  let root = format!("/tmp/ar5iv_adjacency_{pid}");
  let (inc, full) = (
    format!("paper_order_test_inc_{pid}"),
    format!("paper_order_test_full_{pid}"),
  );
  let _: () = redis::cmd("DEL")
    .arg(&inc)
    .arg(&full)
    .query(&mut conn)
    .unwrap();
  let _ = fs::remove_dir_all(&root);

  // Name order puts 9107 (1991) after 2609, so the new month lands mid-sequence, and 9107 holds
  // old-style ids; an empty month and a stray file must be skipped like the full walk does.
  papers(&root, "0001", &["0001.00001", "0001.00002"]);
  papers(&root, "2608", &["2608.00001", "2608.00002", "2608.00003"]);
  papers(&root, "9107", &["astro-ph9107001", "hep-th9107002"]);
  fs::create_dir_all(format!("{root}/2610")).unwrap();
  fs::write(format!("{root}/2608/stray.txt"), "x").unwrap();
  rebuild(&mut conn, &inc, &root).unwrap();

  papers(&root, "2609", &["2609.00001", "2609.00002"]);
  let linked = link_month(&mut conn, &inc, &root, "2609").unwrap();
  let incremental = map(&mut conn, &inc);
  link_month(&mut conn, &inc, &root, "2609").unwrap();
  let relinked = map(&mut conn, &inc);
  rebuild(&mut conn, &full, &root).unwrap();
  let expected = map(&mut conn, &full);

  let _: () = redis::cmd("DEL")
    .arg(&inc)
    .arg(&full)
    .query(&mut conn)
    .unwrap();
  let _ = fs::remove_dir_all(&root);
  assert_eq!(linked, 2, "both new papers are linked");
  assert_eq!(incremental, expected, "incremental link == full rebuild");
  assert_eq!(relinked, expected, "re-linking a month is idempotent");
}
