use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SupportedClientFeatures {
  // Supports Collab Params serialization using Protobuf
  CollabParamsProtobuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerInfoResponseItem {
  pub supported_client_features: Vec<SupportedClientFeatures>,
  pub minimum_supported_client_version: Option<String>,
  pub appflowy_web_url: String,
}

/// Admin console capabilities advertised by the server.
///
/// Variant names are part of the JSON contract shared with AppFlowy Admin.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SuperAdminTags {
  UserManagement,
  UpgradePlan,
  PublishPageManagement,
  CommercialLicenseSubscription,
  CollabDocument,
  SearchDiagnostics,
  Permission,
  AI,
  MCP,
  ServerHealth,
}

#[cfg(test)]
mod tests {
  use serde_json::Value;

  use super::SuperAdminTags;

  #[test]
  fn mcp_admin_tag_serializes_stably() {
    assert_eq!(
      serde_json::to_value(SuperAdminTags::MCP).unwrap(),
      Value::String("MCP".to_string())
    );
  }
}
