use std::time::Duration;

use app_error::AppError;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Live ARS role check for a registered wiki. There is deliberately no grant
/// cache: a role change must take effect on the next read or edit.
#[derive(Clone)]
pub(super) struct WikiAccess {
  pool: PgPool,
  client: reqwest::Client,
  endpoint: Option<String>,
  key: String,
}

impl WikiAccess {
  pub(super) fn from_env(pool: PgPool) -> Result<Self, AppError> {
    let endpoint = std::env::var("ARS_WIKI_RPC_URL")
      .ok()
      .filter(|s| !s.is_empty());
    let key = std::env::var("ARS_MEMBERSHIP_SERVICE_KEY").unwrap_or_default();
    if let Some(ref raw) = endpoint {
      let url = reqwest::Url::parse(raw)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid ARS wiki URL")))?;
      if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || key.is_empty()
      {
        return Err(AppError::Internal(anyhow::anyhow!(
          "Incomplete ARS wiki access configuration"
        )));
      }
    }
    let client = reqwest::Client::builder()
      .timeout(Duration::from_secs(5))
      .redirect(reqwest::redirect::Policy::none())
      .build()
      .map_err(|_| AppError::Internal(anyhow::anyhow!("ARS wiki client unavailable")))?;
    Ok(Self {
      pool,
      client,
      endpoint,
      key,
    })
  }

  pub(super) async fn allows(
    &self,
    uid: &i64,
    workspace: &Uuid,
    page: &Uuid,
  ) -> Result<bool, AppError> {
    let row = sqlx::query(
      "SELECT workspace_id, audience_kind, chapter_id, named_user_uuid, archived_at IS NOT NULL AS archived \
       FROM ars_wiki_page WHERE page_id = $1",
    )
    .bind(page)
    .fetch_optional(&self.pool)
    .await?;
    let Some(row) = row else {
      return Ok(true);
    };
    if row.get::<Uuid, _>("workspace_id") != *workspace {
      return Ok(false);
    }
    if row.get::<bool, _>("archived") {
      return Ok(false);
    }
    let Some(endpoint) = &self.endpoint else {
      return Ok(false);
    };
    let user: Uuid = sqlx::query_scalar("SELECT uuid FROM af_user WHERE uid = $1")
      .bind(uid)
      .fetch_one(&self.pool)
      .await?;
    let response = self
      .client
      .post(endpoint)
      .header("apikey", &self.key)
      .bearer_auth(&self.key)
      .json(&serde_json::json!({
        "p_user": user,
        "p_audience": row.get::<String, _>("audience_kind"),
        "p_chapter": row.get::<Option<Uuid>, _>("chapter_id"),
        "p_named_user": row.get::<Option<Uuid>, _>("named_user_uuid"),
      }))
      .send()
      .await
      .map_err(|_| AppError::NotEnoughPermissions)?;
    if !response.status().is_success() {
      return Ok(false);
    }
    response
      .json::<bool>()
      .await
      .map_err(|_| AppError::NotEnoughPermissions)
  }
}
