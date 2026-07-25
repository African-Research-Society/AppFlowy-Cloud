# Admin capability tags

`GET /api/admin/tags` returns read-only capability metadata used by AppFlowy Admin to decide
which navigation entries and pages to show. The endpoint intentionally mirrors the public
capability endpoint in AppFlowy Cloud Premium: it does not expose user, workspace, license, or
other administrative data, and it does not authorize any operation.

The non-commercial server always advertises `MCP`. AppFlowy Admin may therefore expose its MCP
tool console for this deployment. `UserManagement` and `UpgradePlan` are also returned to preserve
the Admin client's existing fetch-failure baseline. Other tags are deliberately omitted because
this server does not provide their corresponding Admin APIs.

Tags are a UI-discoverability contract, not a security boundary. Any API or MCP operation exposed
by the server must continue to enforce authentication, authorization, and workspace permissions
independently.
