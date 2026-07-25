use actix_web::{web, Scope};
use shared_entity::dto::server_info_dto::SuperAdminTags;
use shared_entity::response::{AppResponse, JsonAppResponse};

/// Returns the read-only capability metadata consumed by AppFlowy Admin.
pub fn admin_scope() -> Scope {
  web::scope("/api/admin").service(web::resource("/tags").route(web::get().to(admin_tags_handler)))
}

async fn admin_tags_handler() -> JsonAppResponse<Vec<SuperAdminTags>> {
  AppResponse::Ok()
    .with_data(noncommercial_admin_tags())
    .into()
}

fn noncommercial_admin_tags() -> Vec<SuperAdminTags> {
  vec![
    SuperAdminTags::UserManagement,
    SuperAdminTags::UpgradePlan,
    SuperAdminTags::MCP,
  ]
}

#[cfg(test)]
mod tests {
  use actix_web::{http::StatusCode, test, App};
  use serde_json::json;

  use super::admin_scope;

  #[actix_web::test]
  async fn admin_tags_endpoint_returns_exact_noncommercial_capabilities() {
    let app = test::init_service(App::new().service(admin_scope())).await;
    let request = test::TestRequest::get().uri("/api/admin/tags").to_request();

    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(response).await;
    assert_eq!(
      body,
      json!({
        "data": [
          "UserManagement",
          "UpgradePlan",
          "MCP"
        ],
        "code": 0,
        "message": "Operation completed successfully."
      })
    );
  }
}
