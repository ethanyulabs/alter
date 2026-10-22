//! JSON-RPC 2.0 的解析、分发与错误格式：`error.data` 固定为 `{code, message, details?}`。

use alter_core::protocol::{
    DEFAULT_PAGE_BUDGET, ErrorCode, HelloParams, HelloResult, HelloSpace, PROTOCOL_VERSION,
    RpcErrorData, SpaceListResult,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::Gateway;
use crate::tabs::OpError;

/// JSON-RPC 的错误：`code` 是 JSON-RPC 层的整数，`data` 是业务错误。
pub(crate) struct RpcError {
    code: i64,
    data: RpcErrorData,
}

impl RpcError {
    /// 业务错误（-32000）。
    pub(crate) fn new(code: ErrorCode, message: impl Into<String>) -> RpcError {
        RpcError {
            code: -32000,
            data: RpcErrorData {
                code,
                message: message.into(),
                details: None,
            },
        }
    }

    /// 带 `details` 的业务错误。
    pub(crate) fn with_details(
        code: ErrorCode,
        message: impl Into<String>,
        details: Value,
    ) -> RpcError {
        let mut e = RpcError::new(code, message);
        e.data.details = Some(details);
        e
    }

    /// 参数不合法（-32602）。
    pub(crate) fn invalid_params(message: impl Into<String>) -> RpcError {
        RpcError {
            code: -32602,
            ..RpcError::new(ErrorCode::InvalidArgument, message)
        }
    }

    /// 业务错误码的字面量（回执日志用）。
    pub(crate) fn code_str(&self) -> String {
        serde_json::to_value(self.data.code)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    }

    pub(crate) fn internal(e: impl std::fmt::Display) -> RpcError {
        RpcError::new(ErrorCode::Internal, e.to_string())
    }
}

/// 处理一条请求，返回要写回的响应；通知（没有 id）不回。
/// `out` 是这个连接的写出队列（只推给发起方的通知用它）。
pub(super) async fn handle(
    gateway: &Gateway,
    text: &str,
    out: &tokio::sync::mpsc::UnboundedSender<String>,
) -> Option<String> {
    let req: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            return Some(error_response(
                Value::Null,
                -32700,
                ErrorCode::InvalidArgument,
                &e.to_string(),
            ));
        }
    };
    let id = req.get("id").cloned();
    let Some(method) = req.get("method").and_then(Value::as_str) else {
        return Some(error_response(
            id.unwrap_or(Value::Null),
            -32600,
            ErrorCode::InvalidArgument,
            "缺少 method",
        ));
    };
    let params = req.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = call(gateway, method, params, out).await;
    let id = id?;
    Some(match result {
        Ok(value) => json!({"jsonrpc": "2.0", "id": id, "result": value}).to_string(),
        Err(e) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": e.code, "message": e.data.message, "data": e.data},
        })
        .to_string(),
    })
}

fn error_response(id: Value, code: i64, data_code: ErrorCode, message: &str) -> String {
    let data = RpcErrorData {
        code: data_code,
        message: message.to_owned(),
        details: None,
    };
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message, "data": data}})
        .to_string()
}

/// 把 `params` 解析为方法的参数结构；失败时的说明里带上缺少或出错的字段名。
pub(super) fn parse<T: DeserializeOwned>(params: Value) -> Result<T, RpcError> {
    serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))
}

impl From<OpError> for RpcError {
    fn from(e: OpError) -> RpcError {
        RpcError {
            code: -32000,
            data: RpcErrorData {
                code: e.code,
                message: e.message,
                details: e.details,
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskSpaceCreateParams {
    name: String,
    #[serde(default)]
    space_id: Option<String>,
}

#[derive(Deserialize)]
struct IdParams {
    id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdoptParams {
    id: i64,
    tab_id: String,
}

#[derive(Deserialize)]
pub(super) struct PageParams {
    pub id: i64,
    pub label: String,
}

#[derive(Deserialize)]
struct FinishParams {
    id: i64,
    #[serde(default)]
    keep: Vec<String>,
}

fn to_value<T: serde::Serialize>(v: T) -> Result<Value, RpcError> {
    serde_json::to_value(v).map_err(RpcError::internal)
}

async fn call(
    gateway: &Gateway,
    method: &str,
    params: Value,
    out: &tokio::sync::mpsc::UnboundedSender<String>,
) -> Result<Value, RpcError> {
    match method {
        "hello" => {
            let p: HelloParams = parse(params)?;
            if p.protocol_version != PROTOCOL_VERSION {
                return Err(RpcError::new(
                    ErrorCode::ProtocolVersion,
                    format!(
                        "协议版本不一致：客户端 {}，alterd {PROTOCOL_VERSION}",
                        p.protocol_version
                    ),
                ));
            }
            let spaces = gateway
                .with_tabs(|tabs| (tabs.space_summaries(), false))
                .await?
                .map_err(RpcError::internal)?;
            let result = HelloResult {
                protocol_version: PROTOCOL_VERSION,
                page_budget: DEFAULT_PAGE_BUDGET,
                spaces: spaces
                    .into_iter()
                    .map(|s| HelloSpace {
                        id: s.id,
                        name: s.name,
                    })
                    .collect(),
            };
            Ok(serde_json::to_value(result).map_err(RpcError::internal)?)
        }
        "space.list" => {
            let spaces = gateway
                .with_tabs(|tabs| (tabs.space_summaries(), false))
                .await?
                .map_err(RpcError::internal)?;
            Ok(serde_json::to_value(SpaceListResult { spaces }).map_err(RpcError::internal)?)
        }
        "taskSpace.create" => {
            let p: TaskSpaceCreateParams = parse(params)?;
            to_value(
                gateway
                    .op(move |t| t.task_space_create(&p.name, p.space_id.as_deref()))
                    .await?,
            )
        }
        "taskSpace.resume" => {
            let p: IdParams = parse(params)?;
            to_value(gateway.op(move |t| t.task_space_resume(p.id)).await?)
        }
        "taskSpace.list" => {
            let list = gateway.op(|t| t.task_space_list()).await?;
            Ok(json!({ "taskSpaces": list }))
        }
        "taskSpace.newPage" => {
            let p: IdParams = parse(params)?;
            let _turn = gateway.queue(p.id).lock_owned().await;
            let label = gateway.op(move |t| t.task_space_new_page(p.id)).await?;
            Ok(json!({ "label": label }))
        }
        "taskSpace.tabs" => {
            let p: IdParams = parse(params)?;
            let tabs = gateway.op(move |t| t.task_space_tabs(p.id)).await?;
            Ok(json!({ "tabs": tabs }))
        }
        "taskSpace.adopt" => {
            let p: AdoptParams = parse(params)?;
            let _turn = gateway.queue(p.id).lock_owned().await;
            let label = gateway
                .op(move |t| t.task_space_adopt(p.id, &p.tab_id))
                .await?;
            Ok(json!({ "label": label }))
        }
        "taskSpace.release" => {
            let p: PageParams = parse(params)?;
            let _turn = gateway.queue(p.id).lock_owned().await;
            gateway
                .op(move |t| t.task_space_release(p.id, &p.label))
                .await?;
            Ok(json!({}))
        }
        "taskSpace.finish" => {
            let p: FinishParams = parse(params)?;
            // 等这个 TaskSpace 正在执行的页面动作结束，再关掉它的页面
            let _turn = gateway.queue(p.id).lock_owned().await;
            let (closed, kept) = gateway
                .op(move |t| t.task_space_finish(p.id, &p.keep))
                .await?;
            Ok(json!({ "closed": closed, "kept": kept }))
        }
        "taskSpace.handOff" => {
            let p: IdParams = parse(params)?;
            let _turn = gateway.queue(p.id).lock_owned().await;
            gateway.op(move |t| t.task_space_hand_off(p.id)).await?;
            Ok(json!({}))
        }
        "taskSpace.takeOver" => {
            let p: IdParams = parse(params)?;
            let _turn = gateway.queue(p.id).lock_owned().await;
            let r = gateway.op(move |t| t.task_space_take_over(p.id)).await?;
            serde_json::to_value(r).map_err(RpcError::internal)
        }
        "taskSpace.cdp" => {
            #[derive(Deserialize)]
            struct TaskSpaceCdpParams {
                id: i64,
                method: String,
                #[serde(default)]
                params: Value,
            }
            let p: TaskSpaceCdpParams = parse(params)?;
            // 排队前订阅接管信号（任何 TaskSpace 被接管都中止，包括自己），队列锁后检查
            let mut takeover = gateway.takeover.subscribe_global();
            let _turn = gateway.queue(p.id).lock_owned().await;
            let id = p.id;
            gateway.op(move |t| t.ensure_raw_cdp_allowed(id)).await?;
            let params = if p.params.is_null() {
                json!({})
            } else {
                p.params
            };
            // 浏览器级（Target、Browser 域）：不带页面会话
            // 接管信号随命令进入写出队列：写出之前接管了也不发
            let guard = takeover.clone();
            let sent = tokio::select! {
                biased;
                _ = takeover.changed() => return Err(super::page::raw_cdp_interrupted()),
                r = gateway.cdp.send_guarded(None, &p.method, params, guard) => r?,
            };
            Ok(json!({"result": sent}))
        }
        "import.run" => super::imports::run(gateway, params, out).await,
        "import.status" => super::imports::status(gateway, params).await,
        "page.goto" | "page.reload" | "page.info" | "page.url" | "page.title" | "page.snapshot"
        | "page.click" | "page.dblclick" | "page.hover" | "page.focus" | "page.fill"
        | "page.press" | "page.selectOption" | "page.dragAndDrop" | "page.setInputFiles"
        | "page.evaluate" | "page.dialog" | "page.screenshot" | "page.waitFor" | "page.cdp"
        | "page.mouse" | "page.keyboard" | "page.fetch" | "page.close" => {
            super::page::call(gateway, method, params).await
        }
        _ => Err(RpcError {
            code: -32601,
            ..RpcError::new(
                ErrorCode::InvalidArgument,
                format!("没有这个方法：{method}"),
            )
        }),
    }
}
