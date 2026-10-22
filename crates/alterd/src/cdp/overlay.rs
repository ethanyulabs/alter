//! agent 叠层（`overlay.js`）：在页面主框架的隔离 world 里执行，页面的 CSP 与脚本都碰不到；
//! 叠层 `pointer-events: none`，不拦截用户的鼠标事件。

use serde_json::json;

use super::{CdpError, CdpMux};

const WORLD: &str = "alter-overlay";
const SCRIPT: &str = include_str!("overlay.js");

/// 在页面（`target` 也是主框架的 frameId）的隔离 world 里执行叠层脚本后跟一句调用。
async fn call(cdp: &CdpMux, session: &str, target: &str, expr: &str) -> Result<(), CdpError> {
    let world = cdp
        .send(
            Some(session),
            "Page.createIsolatedWorld",
            json!({"frameId": target, "worldName": WORLD}),
        )
        .await?;
    let r = cdp
        .send(
            Some(session),
            "Runtime.evaluate",
            json!({
                "expression": format!("{SCRIPT}\n{expr}"),
                "contextId": world["executionContextId"],
            }),
        )
        .await?;
    if let Some(ex) = r.get("exceptionDetails") {
        return Err(CdpError::Protocol(format!("叠层脚本出错：{ex}")));
    }
    Ok(())
}

/// 在视口坐标 (x, y) 显示 agent 光标与动作文字。
pub async fn show(
    cdp: &CdpMux,
    session: &str,
    target: &str,
    x: f64,
    y: f64,
    text: &str,
) -> Result<(), CdpError> {
    let expr = format!("__alterOverlayShow({x}, {y}, {});", json!(text));
    call(cdp, session, target, &expr).await
}

/// 隐藏叠层（转为用户控制时）。
pub async fn hide(cdp: &CdpMux, session: &str, target: &str) -> Result<(), CdpError> {
    call(cdp, session, target, "__alterOverlayHide();").await
}

/// 截图前暂时藏起。
pub async fn suspend(cdp: &CdpMux, session: &str, target: &str) -> Result<(), CdpError> {
    call(cdp, session, target, "__alterOverlaySuspend();").await
}

/// 截图后按原来的可见状态恢复。
pub async fn resume(cdp: &CdpMux, session: &str, target: &str) -> Result<(), CdpError> {
    call(cdp, session, target, "__alterOverlayResume();").await
}
