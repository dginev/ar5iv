use crate::assemble_asset::{assemble_log, assemble_paper, assemble_paper_asset};
use rand::seq::SliceRandom;
use regex::Regex;
use rocket::fs::NamedFile;
use rocket::http::ContentType;
use rocket::tokio::sync::Mutex;
use rocket_db_pools::deadpool_redis::redis::aio;
use rocket_db_pools::deadpool_redis::redis::{cmd, RedisError};
use rocket_db_pools::Connection;
use rocket_db_pools::{deadpool_redis, Database};
use std::path::Path;
use std::sync::LazyLock;

pub const TEN_MIB: usize = 10_485_760; // bytes; the per-item cache cap
pub const SIXTY_FOUR_MIB: u64 = 67_108_864; // hard cap for buffering a single ZIP asset into RAM

static ARXIV_ID_VERSION: LazyLock<Regex> = LazyLock::new(|| Regex::new("v\\d\\d?$").unwrap());

/// Namespaced cache keys: papers, assets and conversion logs live in disjoint
/// keyspaces, so that e.g. an asset literally named like the conversion log
/// can never poison the log cache (or vice versa).
pub fn paper_key(id_arxiv: &str) -> String {
  format!("p:{id_arxiv}")
}
pub fn asset_key(id_arxiv: &str, filename: &str) -> String {
  format!("a:{id_arxiv}/{filename}")
}
pub fn log_key(id_arxiv: &str) -> String {
  format!("l:{id_arxiv}")
}

#[derive(Database)]
#[database("memdb")]
pub struct Cache(deadpool_redis::Pool);

pub async fn set_cached(
  conn: &mut aio::MultiplexedConnection,
  key: &str,
  val: &str,
) -> Result<(), ()> {
  cmd("SET")
    .arg(&[key, val])
    .query_async::<_, ()>(conn)
    .await
    .map_err(|_| ())
}

pub async fn get_cached(conn: &mut aio::MultiplexedConnection, key: &str) -> Result<String, ()> {
  let value: Result<String, ()> = cmd("GET")
    .arg(&[key])
    .query_async::<_, String>(conn)
    .await
    .map_err(|_| ());
  value
}

pub async fn set_cached_asset(
  conn: &mut aio::MultiplexedConnection,
  key: &str,
  val: &[u8],
) -> Result<(), ()> {
  cmd("SET")
    .arg(key)
    .arg(val)
    .query_async::<_, ()>(conn)
    .await
    .map_err(|_| ())
}
pub async fn get_cached_asset(
  conn: &mut aio::MultiplexedConnection,
  key: &str,
) -> Result<Vec<u8>, ()> {
  let result: Result<Vec<u8>, ()> = cmd("GET")
    .arg(&[key])
    .query_async::<_, Vec<u8>>(conn)
    .await
    .map_err(|_| ());
  match result {
    Ok(value) => {
      // guard: a successful asset get should not be empty
      if value.is_empty() {
        Err(())
      } else {
        Ok(value)
      }
    }
    Err(e) => Err(e),
  }
}

pub async fn hget_cached(
  conn: &mut aio::MultiplexedConnection,
  hash: &str,
  key: &str,
) -> Result<String, ()> {
  let value: Result<String, ()> = cmd("HGET")
    .arg(hash)
    .arg(key)
    .query_async::<_, String>(conn)
    .await
    .map_err(|_| ());
  value
}

pub async fn assemble_paper_with_cache(
  mut conn_opt: Option<Connection<Cache>>,
  field_opt: Option<&str>,
  id_raw: &str,
) -> Option<String> {
  let id = ARXIV_ID_VERSION.replace(id_raw, "");
  let cached = match conn_opt {
    Some(ref mut conn) => {
      let key = paper_key(&build_arxiv_id(&field_opt, &id));
      get_cached(&mut *conn, &key).await.unwrap_or_default()
    }
    None => String::default(),
  };
  if !cached.is_empty() {
    Some(cached)
  } else {
    assemble_paper(conn_opt, field_opt, &id).await
  }
}

pub async fn assemble_paper_asset_with_cache(
  mut conn_opt: Option<Connection<Cache>>,
  field_opt: Option<&str>,
  id_raw: &str,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  let id = ARXIV_ID_VERSION.replace(id_raw, "");
  let key = asset_key(&build_arxiv_id(&field_opt, &id), filename);
  let cached = match conn_opt {
    Some(ref mut conn) => get_cached_asset(&mut *conn, &key).await.unwrap_or_default(),
    None => Vec::new(),
  };
  let asset_opt = if !cached.is_empty() {
    Ok(cached)
  } else if let Some(asset) = assemble_paper_asset(field_opt, &id, filename).await {
    if asset.is_empty() {
      Err(
        NamedFile::open(Path::new("assets/missing_image.png"))
          .await
          .ok(),
      )
    } else {
      if asset.len() <= TEN_MIB {
        // cap cache items at 10 MiB
        if let Some(ref mut conn) = conn_opt {
          set_cached_asset(&mut *conn, &key, &asset).await.ok();
        }
      }
      Ok(asset)
    }
  } else {
    Err(
      NamedFile::open(Path::new("assets/missing_image.png"))
        .await
        .ok(),
    )
  };

  asset_opt.map(|asset| {
    (
      ContentType::from_extension(filename.split('.').next_back().unwrap_or("png"))
        .unwrap_or(ContentType::PNG),
      asset,
    )
  })
}

pub async fn assemble_log_with_cache(
  mut conn_opt: Option<Connection<Cache>>,
  field_opt: Option<&str>,
  id_raw: &str,
) -> Option<String> {
  let id = ARXIV_ID_VERSION.replace(id_raw, "");
  let key = log_key(&build_arxiv_id(&field_opt, &id));
  let cached = match conn_opt {
    Some(ref mut conn) => get_cached(&mut *conn, &key).await.unwrap_or_default(),
    None => String::new(),
  };
  if !cached.is_empty() {
    Some(cached)
  } else if let Some(paper) = assemble_log(field_opt, &id).await {
    // cap cache items at 10 MiB
    if !paper.is_empty() && paper.len() <= TEN_MIB {
      if let Some(mut conn) = conn_opt {
        set_cached(&mut conn, &key, paper.as_str()).await.ok();
      }
    }
    Some(paper)
  } else {
    None
  }
}

/// We universally use the arxiv id scheme for both arxiv id refs and cache keys.
pub fn build_arxiv_id(field_opt: &Option<&str>, id: &str) -> String {
  if let Some(ref field) = field_opt {
    format!("{field}/{id}")
  } else {
    id.to_owned()
  }
}

/// A shuffle-bag over all known article ids, for the /feeling_lucky route.
/// Seeded lazily from Redis, reseeded (and reshuffled) when drained.
pub struct LuckyStore(Mutex<Vec<String>>);
impl LuckyStore {
  pub fn new() -> Self {
    LuckyStore(Mutex::new(Vec::new()))
  }
  pub async fn get(&self, conn: &mut aio::MultiplexedConnection) -> Option<String> {
    let mut bag = self.0.lock().await;
    if bag.is_empty() {
      // (re)seed from Redis -- once per full rotation of the article set
      let all_articles_result: Result<Vec<String>, RedisError> = cmd("HKEYS")
        .arg("paper_order")
        .query_async::<_, Vec<String>>(conn)
        .await;
      *bag = all_articles_result.unwrap_or_default();
      let mut rng = rand::rng();
      bag.shuffle(&mut rng);
    }
    bag.pop()
  }
}
impl Default for LuckyStore {
  fn default() -> Self {
    Self::new()
  }
}
