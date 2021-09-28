use crate::dirty_templates::{assemble_paper, assemble_paper_asset};
use rocket::http::ContentType;
use rocket_db_pools::deadpool_redis::redis::cmd;
use rocket_db_pools::deadpool_redis::ConnectionWrapper;

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

pub async fn assemble_paper_with_cache(
  conn: &mut ConnectionWrapper,
  field_opt: Option<String>,
  id: &str,
) -> String {
  let key = match field_opt {
    Some(ref field) => field.to_string() + id,
    None => id.to_string(),
  };
  let cached = get_cached(&mut *conn, &key).await.unwrap_or_default();
  if !cached.is_empty() {
    cached
  } else {
    let paper = assemble_paper(conn, field_opt, id).await;
    set_cached(&mut *conn, &key, paper.as_str()).await.ok();
    paper
  }
}

pub async fn assemble_paper_asset_with_cache(
  conn: &mut ConnectionWrapper,
  field_opt: Option<String>,
  id: &str,
  filename: &str,
) -> Option<(ContentType, Vec<u8>)> {
  let key = match field_opt {
    Some(ref field) => field.to_string() + id + "/" + filename,
    None => id.to_string() + "/" + filename,
  };
  let cached = get_cached_asset(&mut *conn, &key).await.unwrap_or_default();
  let asset_opt = if !cached.is_empty() {
    Some(cached)
  } else if let Some(asset) = assemble_paper_asset(field_opt, id, filename).await {
    if asset.is_empty() {
      None
    } else {
      set_cached_asset(&mut *conn, &key, &asset).await.ok();
      Some(asset)
    }
  } else {
    None
  };

  asset_opt.map(|asset| {
    (
      ContentType::from_extension(filename.split('.').last().unwrap_or("png"))
        .unwrap_or(ContentType::PNG),
      asset,
    )
  })
}
