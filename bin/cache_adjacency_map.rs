use ar5iv::paper_order::{link_month, rebuild, AR5IV_PAPERS_ROOT_DIR};

/// `cache_adjacency_map` rebuilds the whole prev/next map; `cache_adjacency_map <YYMM>` links just
/// that month (the monthly release), in seconds instead of a full tree walk. `PAPER_ORDER_KEY`
/// overrides the hash key (e.g. a scratch key to verify against production's).
fn main() -> redis::RedisResult<()> {
  let client = redis::Client::open("redis://127.0.0.1/")?;
  let mut conn = client.get_connection()?;
  let key = std::env::var("PAPER_ORDER_KEY").unwrap_or_else(|_| String::from("paper_order"));
  match std::env::args().nth(1) {
    Some(month) => {
      let linked = link_month(&mut conn, &key, &AR5IV_PAPERS_ROOT_DIR, &month)?;
      println!("linked {linked} papers of {month}");
      Ok(())
    }
    None => rebuild(&mut conn, &key, &AR5IV_PAPERS_ROOT_DIR),
  }
}
