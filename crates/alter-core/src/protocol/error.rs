use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Gateway 与 CLI 共用的错误码，取值见 .dev-flow/infra/api.md。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum ErrorCode {
    #[serde(rename = "E_UNAUTHORIZED")]
    Unauthorized,
    #[serde(rename = "E_PROTOCOL_VERSION")]
    ProtocolVersion,
    #[serde(rename = "E_SPACE_NOT_FOUND")]
    SpaceNotFound,
    #[serde(rename = "E_TASK_SPACE_NOT_FOUND")]
    TaskSpaceNotFound,
    #[serde(rename = "E_PAGE_NOT_FOUND")]
    PageNotFound,
    #[serde(rename = "E_PAGE_BUDGET")]
    PageBudget,
    #[serde(rename = "E_USER_CONTROL")]
    UserControl,
    #[serde(rename = "E_AMBIGUOUS_SELECTOR")]
    AmbiguousSelector,
    #[serde(rename = "E_NO_MATCH")]
    NoMatch,
    #[serde(rename = "E_TIMEOUT")]
    Timeout,
    #[serde(rename = "E_DIALOG_OPEN")]
    DialogOpen,
    #[serde(rename = "E_IMPORT_SOURCE_NOT_FOUND")]
    ImportSourceNotFound,
    #[serde(rename = "E_KEYCHAIN_DENIED")]
    KeychainDenied,
    #[serde(rename = "E_INTERNAL")]
    Internal,
    #[serde(rename = "E_TAB_NOT_FOUND")]
    TabNotFound,
    #[serde(rename = "E_INVALID_ARGUMENT")]
    InvalidArgument,
}
