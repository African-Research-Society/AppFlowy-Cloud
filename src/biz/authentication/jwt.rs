use crate::state::AppState;
use actix_http::Payload;
use actix_web::{web::Data, FromRequest, HttpRequest};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use gotrue_entity::gotrue_jwt::GoTrueJWTClaims;
use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};
use sqlx::types::{uuid, Uuid};
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::str::FromStr;
use tracing::instrument;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUuid(uuid::Uuid);

impl UserUuid {
  pub fn from_auth(auth: Authorization) -> Result<Self, actix_web::Error> {
    Ok(Self(auth.uuid()?))
  }
}

impl Deref for UserUuid {
  type Target = uuid::Uuid;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Clone, Debug)]
pub struct UserToken(pub String);
impl UserToken {
  pub fn from_auth(auth: Authorization) -> Result<Self, actix_web::Error> {
    Ok(Self(auth.token))
  }
}

impl Display for UserToken {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0)
  }
}

impl FromRequest for UserUuid {
  type Error = actix_web::Error;

  type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

  fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
    let req = req.clone();
    Box::pin(async move { UserUuid::from_auth(get_auth_from_request(&req).await?) })
  }
}

// For cases where the handler itself will handle the request differently
// based on whether the user is authenticated or not
pub struct OptionalUserUuid(Option<UserUuid>);

impl FromRequest for OptionalUserUuid {
  type Error = actix_web::Error;

  type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

  fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
    let req = req.clone();
    Box::pin(async move {
      Ok(OptionalUserUuid(
        get_auth_from_request(&req)
          .await
          .ok()
          .and_then(|auth| UserUuid::from_auth(auth).ok()),
      ))
    })
  }
}

impl OptionalUserUuid {
  pub fn as_uuid(&self) -> Option<uuid::Uuid> {
    self.0.as_deref().map(|uuid| uuid.to_owned())
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Authorization {
  pub token: String,
  pub claims: GoTrueJWTClaims,
}

impl Authorization {
  pub fn uuid(&self) -> Result<uuid::Uuid, actix_web::Error> {
    let sub = self.claims.sub.as_deref();
    match sub {
      None => Err(actix_web::error::ErrorUnauthorized(
        "Invalid Authorization header, missing sub(uuid)",
      )),
      Some(sub) => match Uuid::from_str(sub) {
        Ok(uuid) => Ok(uuid),
        Err(_) => Err(actix_web::error::ErrorUnauthorized(format!(
          "Invalid Authorization header, invalid sub: {}",
          sub
        ))),
      },
    }
  }
}

impl FromRequest for Authorization {
  type Error = actix_web::Error;

  type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

  fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
    let req = req.clone();
    Box::pin(async move { get_auth_from_request(&req).await })
  }
}

async fn get_auth_from_request(req: &HttpRequest) -> Result<Authorization, actix_web::Error> {
  let jwt_secret_data =
    req
      .app_data::<Data<Secret<String>>>()
      .ok_or(actix_web::error::ErrorInternalServerError(
        "jwt secret not found",
      ))?;
  let bearer = req
    .headers()
    .get("Authorization")
    .ok_or(actix_web::error::ErrorUnauthorized(
      "No Authorization header",
    ))?;

  let bearer_str = bearer
    .to_str()
    .map_err(actix_web::error::ErrorUnauthorized)?;

  let token = bearer_str
    .strip_prefix("Bearer ")
    .ok_or(actix_web::error::ErrorUnauthorized(
      "Invalid Authorization header, missing Bearer",
    ))?;

  let state =
    req
      .app_data::<Data<AppState>>()
      .ok_or(actix_web::error::ErrorInternalServerError(
        "Application state missing",
      ))?;
  authorization_from_token(token, jwt_secret_data, state).await
}

#[instrument(level = "trace", skip_all, err)]
pub async fn authorization_from_token(
  token: &str,
  jwt_secret: &Data<Secret<String>>,
  state: &Data<AppState>,
) -> Result<Authorization, actix_web::Error> {
  let claims = gotrue_jwt_claims_from_token(token, jwt_secret)?;
  if std::env::var_os("ARS_AUTH_ISSUER").is_some() {
    let user = tokio::time::timeout(Duration::from_secs(5), state.gotrue_client.user_info(token))
      .await
      .map_err(|_| actix_web::error::ErrorUnauthorized("Session validation unavailable"))?
      .map_err(|_| actix_web::error::ErrorUnauthorized("Session expired or revoked"))?;
    if Some(user.id.as_str()) != claims.sub.as_deref() {
      return Err(actix_web::error::ErrorUnauthorized(
        "Session identity mismatch",
      ));
    }
  }
  Ok(Authorization {
    token: token.to_string(),
    claims,
  })
}

#[instrument(level = "trace", skip_all, err)]
fn gotrue_jwt_claims_from_token(
  token: &str,
  jwt_secret: &Data<Secret<String>>,
) -> Result<GoTrueJWTClaims, actix_web::Error> {
  let claims =
    GoTrueJWTClaims::decode(token, jwt_secret.expose_secret().as_bytes()).map_err(|err| {
      actix_web::error::ErrorUnauthorized(format!("fail to decode token, error:{}", err))
    })?;
  Ok(claims)
}

/// No positive cache across WebSocket checks. A closed channel or ARS outage
/// terminates the connection; the client must refresh and reconnect.
pub fn monitor_ars_session(
  state: &Data<AppState>,
  token: String,
) -> Option<tokio::sync::watch::Receiver<bool>> {
  if std::env::var_os("ARS_AUTH_ISSUER").is_none() {
    return None;
  }
  let client = state.gotrue_client.clone();
  let (sender, receiver) = tokio::sync::watch::channel(true);
  actix::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    loop {
      tokio::select! {
        _ = sender.closed() => break,
        _ = interval.tick() => {
          let valid = matches!(tokio::time::timeout(Duration::from_secs(5), client.user_info(&token)).await, Ok(Ok(_)));
          if sender.send(valid).is_err() || !valid { break; }
        }
      }
    }
  });
  Some(receiver)
}
