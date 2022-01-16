use crate::dirty_templates::{assemble_log, assemble_paper, assemble_paper_asset, LOG_FILENAME};
use rocket::fs::NamedFile;
use rocket::http::ContentType;
use rocket_db_pools::deadpool_redis::redis::{cmd, RedisError};
use rocket_db_pools::deadpool_redis::ConnectionWrapper;
use rocket_db_pools::Connection;
use rocket_db_pools::{deadpool_redis, Database};
use std::path::Path;
use rand::seq::IteratorRandom;

#[derive(Database)]
#[database("memdb")]
pub struct Cache(deadpool_redis::Pool);

pub async fn set_cached(conn: &mut ConnectionWrapper, key: &str, val: &str) -> Result<(), ()> {
  cmd("SET")
    .arg(&[key, val])
    .query_async::<_, ()>(conn)
    .await
    .map_err(|_| ())
}

pub async fn get_cached(conn: &mut ConnectionWrapper, key: &str) -> Result<String, ()> {
  let value: Result<String, ()> = cmd("GET")
    .arg(&[key])
    .query_async::<_, String>(conn)
    .await
    .map_err(|_| ());
  value
}

pub async fn set_cached_asset(
  conn: &mut ConnectionWrapper,
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
pub async fn get_cached_asset(conn: &mut ConnectionWrapper, key: &str) -> Result<Vec<u8>, ()> {
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
  conn: &mut ConnectionWrapper,
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
  id: &str,
) -> Option<String> {
  let cached = match conn_opt {
    Some(ref mut conn) => {
      let key = build_arxiv_id(&field_opt, id);
      get_cached(&mut *conn, &key).await.unwrap_or_default()
    }
    None => String::default(),
  };
  if !cached.is_empty() {
    Some(cached)
  } else {
    assemble_paper(conn_opt, field_opt, id).await
  }
}

pub async fn assemble_paper_asset_with_cache(
  mut conn_opt: Option<Connection<Cache>>,
  field_opt: Option<&str>,
  id: &str,
  filename: &str,
) -> Result<(ContentType, Vec<u8>), Option<NamedFile>> {
  let key = match field_opt {
    Some(ref field) => field.to_string() + id + "/" + filename,
    None => id.to_string() + "/" + filename,
  };
  let cached = match conn_opt {
    Some(ref mut conn) => get_cached_asset(&mut *conn, &key).await.unwrap_or_default(),
    None => Vec::new(),
  };
  let asset_opt = if !cached.is_empty() {
    Ok(cached)
  } else if let Some(asset) = assemble_paper_asset(field_opt, id, filename).await {
    if asset.is_empty() {
      Err(
        NamedFile::open(Path::new("assets/missing_image.png"))
          .await
          .ok(),
      )
    } else {
      if let Some(ref mut conn) = conn_opt {
        set_cached_asset(&mut *conn, &key, &asset).await.ok();
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
  id: &str,
) -> Option<String> {
  let key = build_arxiv_id(&field_opt, id) + "/" + LOG_FILENAME;
  let cached = match conn_opt {
    Some(ref mut conn) => get_cached(&mut *conn, &key).await.unwrap_or_default(),
    None => String::new(),
  };
  if !cached.is_empty() {
    Some(cached)
  } else if let Some(paper) = assemble_log(field_opt, id).await {
    if let Some(mut conn) = conn_opt {
      set_cached(&mut conn, &key, paper.as_str()).await.ok();
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


pub async fn lucky_url(conn: &mut ConnectionWrapper) -> Option<String> {
  // it makes no sense to call this twice due to the size, just put it in a lazy static.
  let all_articles_result: Result<Vec<String>, RedisError> = cmd("HKEYS")
    .arg("paper_order")
    .query_async::<_, Vec<String>>(conn).await;
  let all_article_ids = all_articles_result.unwrap_or_default();
  let mut rng = rand::thread_rng();
  all_article_ids.iter().choose(&mut rng).map(|id| String::from("/html/")+id)
}
