use crate::assemble_asset::{assemble_log, assemble_paper, assemble_paper_asset};
use crate::constants::LOG_FILENAME;
use crossbeam::queue::ArrayQueue;
use rand::seq::{IteratorRandom, SliceRandom};
use regex::Regex;
use rocket::fs::NamedFile;
use rocket::http::ContentType;
use rocket_db_pools::deadpool_redis::redis::aio;
use rocket_db_pools::deadpool_redis::redis::{cmd, RedisError};
use rocket_db_pools::Connection;
use rocket_db_pools::{deadpool_redis, Database};
use std::path::Path;

pub const TEN_MIB: usize = 10_485_760; // u8 is 1 byte
pub const TWO_AND_A_HALF_MIB: usize = 2_621_440; //a char is 4 bytes

lazy_static! {
  static ref ARXIV_ID_VERSION: Regex = Regex::new("v\\d\\d?$").unwrap();
}

#[derive(Database)]
#[database("memdb")]
pub struct Cache(deadpool_redis::Pool);

pub async fn set_cached(conn: &mut aio::Connection, key: &str, val: &str) -> Result<(), ()> {
  cmd("SET")
    .arg(&[key, val])
    .query_async::<_, ()>(conn)
    .await
    .map_err(|_| ())
}

pub async fn get_cached(conn: &mut aio::Connection, key: &str) -> Result<String, ()> {
  let value: Result<String, ()> = cmd("GET")
    .arg(&[key])
    .query_async::<_, String>(conn)
    .await
    .map_err(|_| ());
  value
}

pub async fn set_cached_asset(conn: &mut aio::Connection, key: &str, val: &[u8]) -> Result<(), ()> {
  cmd("SET")
    .arg(key)
    .arg(val)
    .query_async::<_, ()>(conn)
    .await
    .map_err(|_| ())
}
pub async fn get_cached_asset(conn: &mut aio::Connection, key: &str) -> Result<Vec<u8>, ()> {
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

pub async fn hget_cached(conn: &mut aio::Connection, hash: &str, key: &str) -> Result<String, ()> {
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
  use_dom: bool,
) -> Option<String> {
  let id = ARXIV_ID_VERSION.replace(id_raw, "");
  let cached = match conn_opt {
    Some(ref mut conn) => {
      let key = build_arxiv_id(&field_opt, &id);
      get_cached(&mut *conn, &key).await.unwrap_or_default()
    }
    None => String::default(),
  };
  if !cached.is_empty() {
    Some(cached)
  } else {
    assemble_paper(conn_opt, field_opt, &id, use_dom).await
  }
}

pub async fn assemble_paper_asset_with_cache(
  mut conn_opt: Option<Connection<Cache>>,
  field_opt: Option<&str>,
  id_raw: &str,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  let id = ARXIV_ID_VERSION.replace(id_raw, "");
  let key = match field_opt {
    Some(ref field) => field.to_string() + &id + "/" + filename,
    None => id.to_string() + "/" + filename,
  };
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
      if asset.len() <= TEN_MIB { // cap cache items at 10 MiB
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
      ContentType::from_extension(filename.split('.').last().unwrap_or("png"))
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
  let key = build_arxiv_id(&field_opt, &id) + "/" + LOG_FILENAME;
  let cached = match conn_opt {
    Some(ref mut conn) => get_cached(&mut *conn, &key).await.unwrap_or_default(),
    None => String::new(),
  };
  if !cached.is_empty() {
    Some(cached)
  } else if let Some(paper) = assemble_log(field_opt, &id).await {
    // (cap cache items at 10 MiB, where a char is 4 bytes)
    if !paper.is_empty() && paper.len() <= TWO_AND_A_HALF_MIB {
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
    format!("{}/{}", field, id)
  } else {
    id.to_owned()
  }
}

pub async fn lucky_url(conn: &mut aio::Connection) -> Option<String> {
  // it makes no sense to call this twice due to the size, just put it in a lazy static.
  let all_articles_result: Result<Vec<String>, RedisError> = cmd("HKEYS")
    .arg("paper_order")
    .query_async::<_, Vec<String>>(conn)
    .await;
  let all_article_ids = all_articles_result.unwrap_or_default();
  let mut rng = rand::thread_rng();
  all_article_ids
    .iter()
    .choose(&mut rng)
    .map(|id| String::from("/html/") + id)
}

pub struct LuckyStore(ArrayQueue<String>, ArrayQueue<String>);
impl LuckyStore {
  pub fn new() -> Self {
    LuckyStore(ArrayQueue::new(2_000_000), ArrayQueue::new(2_000_000))
  }
  pub async fn get(&self, conn: &mut aio::Connection) -> Option<String> {
    if self.0.is_empty() {
      if self.1.is_empty() {
        // initial call, fill up from Redis
        let all_articles_result: Result<Vec<String>, RedisError> = cmd("HKEYS")
          .arg("paper_order")
          .query_async::<_, Vec<String>>(conn)
          .await;
        let mut all_article_ids = all_articles_result.unwrap_or_default();
        let mut rng = rand::thread_rng();
        all_article_ids.shuffle(&mut rng);
        // seed the thread-safe datastructure
        for id in all_article_ids.into_iter() {
          self.0.push(id).unwrap();
        }
      } else {
        // rotate and reshuffle, the verbose way
        let mut buffer = Vec::new();
        while let Some(id) = self.1.pop() {
          buffer.push(id);
        }
        let mut rng = rand::thread_rng();
        buffer.shuffle(&mut rng);
        for id in buffer.into_iter() {
          self.0.push(id).unwrap();
        }
      }
    }
    if let Some(next) = self.0.pop() {
      self.1.push(next.clone()).unwrap();
      Some(next)
    } else {
      None
    }
  }
}
impl Default for LuckyStore {
  fn default() -> Self {
    Self::new()
  }
}
