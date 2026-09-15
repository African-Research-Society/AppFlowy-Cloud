use std::time::Duration;

use app_error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

use crate::entity::ObjectType;

/// Optional ARS gate, evaluated before the existing AppFlowy policy on every
/// access check. No positive cache: revocation must also affect native clients.
#[derive(Clone)]
pub(super) struct ArsMembership {
  pool: PgPool,
  client: reqwest::Client,
  endpoint: String,
  key: String,
  operator: Option<Uuid>,
}

impl ArsMembership {
  pub(super) fn from_env(pool: PgPool) -> Result<Option<Self>, AppError> {
    let endpoint = std::env::var("ARS_MEMBERSHIP_RPC_URL").unwrap_or_default();
    let key = std::env::var("ARS_MEMBERSHIP_SERVICE_KEY").unwrap_or_default();
    if endpoint.is_empty() && key.is_empty() {
      return Ok(None);
    }
    let url = reqwest::Url::parse(&endpoint)
      .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid ARS membership URL")))?;
    if url.scheme() != "https"
      || key.is_empty()
      || !url.username().is_empty()
      || url.password().is_some()
    {
      return Err(AppError::Internal(anyhow::anyhow!(
        "Incomplete ARS membership configuration"
      )));
    }
    let operator = std::env::var("ARS_WORKSPACE_OPERATOR_UUID")
      .ok()
      .filter(|value| !value.is_empty())
      .map(|value| Uuid::parse_str(&value))
      .transpose()
      .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid ARS workspace operator UUID")))?;
    let client = reqwest::Client::builder()
      .timeout(Duration::from_secs(5))
      .redirect(reqwest::redirect::Policy::none())
      .build()
      .map_err(|_| AppError::Internal(anyhow::anyhow!("ARS membership client unavailable")))?;
    Ok(Some(Self {
      pool,
      client,
      endpoint,
      key,
      operator,
    }))
  }

  pub(super) async fn allows(&self, uid: &i64, object: &ObjectType) -> Result<bool, AppError> {
    let user: Uuid = sqlx::query_scalar("SELECT uuid FROM af_user WHERE uid = $1")
      .bind(uid)
      .fetch_one(&self.pool)
      .await?;
    // The server-held workspace owner credential still has to pass AppFlowy's
    // normal Owner checks. This is not an end-user permission bypass.
    if self.operator == Some(user) {
      return Ok(true);
    }
    let workspace = match object {
      ObjectType::Workspace(id) => {
        Uuid::parse_str(id).map_err(|_| AppError::NotEnoughPermissions)?
      },
      ObjectType::Collab(id) => {
        let oid = Uuid::parse_str(id).map_err(|_| AppError::NotEnoughPermissions)?;
        match sqlx::query_scalar::<_, Uuid>("SELECT workspace_id FROM af_collab WHERE oid = $1")
          .bind(oid)
          .fetch_optional(&self.pool)
          .await?
        {
          Some(workspace) => workspace,
          // New collabs must be authorized through their workspace, not a
          // pre-existing object policy with an unknown workspace.
          None => return Ok(false),
        }
      },
    };
    let response = self
      .client
      .post(&self.endpoint)
      .header("apikey", &self.key)
      .bearer_auth(&self.key)
      .json(&serde_json::json!({ "p_workspace": workspace, "p_user": user }))
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
