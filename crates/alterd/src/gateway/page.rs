//! `page.*`：经 CdpMux 直接对页面发 CDP。同一 TaskSpace 的动作串行；每次调用写一条回执日志
//! （target `receipt`：task_space_id、label、method、duration_ms、ok、error_code）。

use std::time::{Duration, Instant};

use alter_core::protocol::{ErrorCode, Popup, Receipt};
use serde::Deserialize;
use serde_json::{Value, json};

use super::Gateway;
use super::rpc::{PageParams, RpcError, parse};
use crate::cdp::CdpError;

impl From<CdpError> for RpcError {
    fn from(e: CdpError) -> RpcError {
        match e {
            CdpError::Interrupted => RpcError::new(ErrorCode::UserControl, e.to_string()),
            _ => RpcError::new(ErrorCode::Internal, e.to_string()),
        }
    }
}

/// 页面动作的入口：排队、执行、记回执日志。
pub(super) async fn call(gw: &Gateway, method: &str, params: Value) -> Result<Value, RpcError> {
    let base: PageParams = parse(params.clone())?;
    // 元素动作的 3–6 词描述（N3 的叠层用），也写进回执日志
    let action_label = params["actionLabel"].as_str().unwrap_or("").to_owned();
    // 排队之前订阅：排队期间或执行中转为用户控制，都以 E_USER_CONTROL 结束
    let mut cancel = gw.takeover.subscribe(base.id);
    let queue = gw.queue(base.id);
    let _turn = queue.lock().await;
    let started = Instant::now();
    let work = async {
        let id = base.id;
        gw.op(move |t| t.ensure_agent(id)).await?;
        run(gw, method, &base, params).await
    };
    // 这次调用产生的全部 CDP 命令都带上该 TaskSpace 的接管信号：入队后被接管就不再写出
    let work = crate::cdp::GUARD.scope(cancel.clone(), work);
    let result = tokio::select! {
        r = work => r,
        _ = cancel.changed() => Err(RpcError::new(
            ErrorCode::UserControl,
            "用户接管了这个 TaskSpace，命令已中止",
        )),
    };
    let duration_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(_) => tracing::info!(
            target: "receipt",
            task_space_id = base.id,
            label = %base.label,
            method,
            duration_ms,
            ok = true,
            error_code = "",
            action_label = %action_label,
        ),
        Err(e) => tracing::info!(
            target: "receipt",
            task_space_id = base.id,
            label = %base.label,
            method,
            duration_ms,
            ok = false,
            error_code = %e.code_str(),
            action_label = %action_label,
        ),
    }
    result
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GotoParams {
    url: String,
    #[serde(default)]
    wait_until: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReloadParams {
    #[serde(default)]
    wait_until: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

const DEFAULT_NAV_TIMEOUT_MS: u64 = 30_000;

async fn run(
    gw: &Gateway,
    method: &str,
    page: &PageParams,
    params: Value,
) -> Result<Value, RpcError> {
    // 对话框开着时页面的 JS 是停住的（evaluate 会一直等），除了处理对话框、读信息和 CDP 透传都先拒绝
    if !matches!(
        method,
        "page.dialog" | "page.info" | "page.url" | "page.title" | "page.cdp" | "page.close"
    ) {
        let sid = session(gw, page).await?;
        if let Some(d) = open_dialog(gw, &sid) {
            return Err(RpcError::with_details(
                ErrorCode::DialogOpen,
                format!("页面上有未处理的{}对话框：{}", d.kind, d.message),
                json!({"type": d.kind, "message": d.message}),
            ));
        }
    }
    match method {
        "page.goto" => {
            let p: GotoParams = parse(params)?;
            let started = Instant::now();
            let sid = session(gw, page).await?;
            let timeout = Duration::from_millis(p.timeout_ms.unwrap_or(DEFAULT_NAV_TIMEOUT_MS));
            let lifecycle = if p.wait_until.as_deref() == Some("domcontentloaded") {
                "DOMContentLoaded"
            } else {
                "load"
            };
            let mut events = gw.cdp.subscribe();
            let nav = async {
                let r = gw
                    .cdp
                    .send(Some(&sid), "Page.navigate", json!({"url": p.url}))
                    .await?;
                // 下载（以及被别的导航打断）时导航以 ERR_ABORTED 结束，不会再有 load
                if r["errorText"] == "net::ERR_ABORTED" {
                    return Ok(());
                }
                if let Some(err) = r["errorText"].as_str() {
                    return Err(RpcError::new(
                        ErrorCode::Internal,
                        format!("导航失败：{err}"),
                    ));
                }
                // 同文档导航（只改 hash）没有 loaderId，不会再有 load
                let Some(loader) = r["loaderId"].as_str().map(str::to_owned) else {
                    return Ok(());
                };
                loop {
                    match events.recv().await {
                        Ok(ev)
                            if ev.session_id.as_deref() == Some(sid.as_str())
                                && ev.method == "Page.lifecycleEvent"
                                && ev.params["loaderId"] == loader.as_str()
                                && ev.params["name"] == lifecycle =>
                        {
                            return Ok(());
                        }
                        // 加载过程中弹出了对话框：load 要等它关掉才会到，先返回，回执里带上对话框
                        Ok(ev)
                            if ev.session_id.as_deref() == Some(sid.as_str())
                                && ev.method == "Page.javascriptDialogOpening" =>
                        {
                            return Ok(());
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(_) => return Err(RpcError::from(CdpError::NotConnected)),
                    }
                }
            };
            match tokio::time::timeout(timeout, nav).await {
                Ok(r) => r?,
                Err(_) => {
                    return Err(RpcError::new(
                        ErrorCode::Timeout,
                        format!("{} ms 内没有完成导航到 {}", timeout.as_millis(), p.url),
                    ));
                }
            }
            let mut r = receipt(started)?;
            if let Some(d) = open_dialog(gw, &sid) {
                r["dialog"] = serde_json::to_value(d).map_err(RpcError::internal)?;
            }
            Ok(r)
        }
        "page.reload" => {
            let p: ReloadParams = parse(params)?;
            let started = Instant::now();
            let sid = session(gw, page).await?;
            let timeout = Duration::from_millis(p.timeout_ms.unwrap_or(DEFAULT_NAV_TIMEOUT_MS));
            // DOMContentLoaded 不等图片等子资源，load 要等
            let done_event = if p.wait_until.as_deref() == Some("domcontentloaded") {
                "Page.domContentEventFired"
            } else {
                "Page.loadEventFired"
            };
            let mut events = gw.cdp.subscribe();
            let reload = async {
                gw.cdp.send(Some(&sid), "Page.reload", json!({})).await?;
                loop {
                    match events.recv().await {
                        Ok(ev)
                            if ev.session_id.as_deref() == Some(sid.as_str())
                                && ev.method == done_event =>
                        {
                            return Ok::<(), RpcError>(());
                        }
                        Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(_) => return Err(RpcError::from(CdpError::NotConnected)),
                    }
                }
            };
            tokio::time::timeout(timeout, reload)
                .await
                .map_err(|_| RpcError::new(ErrorCode::Timeout, "重新加载超时"))??;
            receipt(started)
        }
        "page.info" | "page.url" | "page.title" => {
            let info = info(gw, page).await?;
            Ok(match method {
                "page.url" => json!({"url": info["url"]}),
                "page.title" => json!({"title": info["title"]}),
                _ => info,
            })
        }
        "page.snapshot" => {
            #[derive(Deserialize)]
            struct SnapshotParams {
                #[serde(default)]
                scope: Option<String>,
                #[serde(default)]
                root: Option<String>,
            }
            let p: SnapshotParams = parse(params)?;
            let sid = session(gw, page).await?;
            let root = match p.scope.as_deref().unwrap_or("full_page") {
                "full_page" => None,
                "subtree" => {
                    let selector = p
                        .root
                        .ok_or_else(|| RpcError::invalid_params("scope 为 subtree 时要给 root"))?;
                    let refs = gw
                        .refs
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .get(&(page.id, page.label.clone()))
                        .cloned()
                        .unwrap_or_default();
                    let el =
                        super::actions::locate(gw, &sid, &selector, &refs, Duration::from_secs(5))
                            .await?;
                    let node = gw
                        .cdp
                        .send(Some(&sid), "DOM.describeNode", json!({"objectId": el}))
                        .await?;
                    node["node"]["backendNodeId"].as_i64()
                }
                other => {
                    return Err(RpcError::invalid_params(format!(
                        "不支持的快照范围：{other}（支持 full_page、subtree）"
                    )));
                }
            };
            let (text, refs) = super::actions::snapshot(gw, &sid, root).await?;
            let count = refs.len();
            gw.refs
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert((page.id, page.label.clone()), refs);
            Ok(json!({"text": text, "refs": count}))
        }
        "page.dialog" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct DialogParams {
                accept: bool,
                #[serde(default)]
                prompt_text: Option<String>,
            }
            let p: DialogParams = parse(params)?;
            let sid = session(gw, page).await?;
            if open_dialog(gw, &sid).is_none() {
                return Err(RpcError::new(ErrorCode::NoMatch, "页面上没有对话框"));
            }
            let mut q = json!({"accept": p.accept});
            if let Some(t) = p.prompt_text {
                q["promptText"] = json!(t);
            }
            gw.cdp
                .send(Some(&sid), "Page.handleJavaScriptDialog", q)
                .await?;
            gw.dialogs
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&sid);
            Ok(json!({}))
        }
        "page.screenshot" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct ShotParams {
                path: String,
                #[serde(default)]
                full_page: bool,
            }
            let p: ShotParams = parse(params)?;
            let path = std::path::PathBuf::from(&p.path);
            if !path.is_absolute() {
                return Err(RpcError::invalid_params(format!(
                    "截图路径必须是绝对路径：{}",
                    p.path
                )));
            }
            let sid = session(gw, page).await?;
            // 截图不拍进 agent 叠层
            let target = gw.cdp.target_of(&sid).await.unwrap_or_default();
            let _ = crate::cdp::overlay::suspend(&gw.cdp, &sid, &target).await;
            let shot = gw
                .cdp
                .send(
                    Some(&sid),
                    "Page.captureScreenshot",
                    json!({"format": "png", "captureBeyondViewport": p.full_page}),
                )
                .await;
            let _ = crate::cdp::overlay::resume(&gw.cdp, &sid, &target).await;
            let r = shot?;
            let png = super::actions::base64_decode(r["data"].as_str().unwrap_or(""))
                .ok_or_else(|| RpcError::internal("截图数据不是合法的 base64"))?;
            let (width, height) =
                super::actions::png_size(&png).ok_or_else(|| RpcError::internal("截图不是 PNG"))?;
            std::fs::write(&path, &png).map_err(|e| {
                RpcError::new(ErrorCode::InvalidArgument, format!("写截图失败：{e}"))
            })?;
            Ok(json!({"path": p.path, "width": width, "height": height}))
        }
        "page.waitFor" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct WaitParams {
                kind: String,
                arg: Value,
                #[serde(default)]
                timeout_ms: Option<u64>,
            }
            let p: WaitParams = parse(params)?;
            let sid = session(gw, page).await?;
            let timeout = Duration::from_millis(p.timeout_ms.unwrap_or(DEFAULT_NAV_TIMEOUT_MS));
            super::actions::wait_for(gw, &sid, &p.kind, &p.arg, timeout).await?;
            Ok(json!({"matched": true}))
        }
        "page.mouse" => {
            #[derive(Deserialize)]
            struct MouseParams {
                op: String,
                #[serde(default)]
                args: Value,
            }
            let p: MouseParams = parse(params)?;
            let started = Instant::now();
            let sid = session(gw, page).await?;
            super::actions::raw_mouse(gw, &sid, &p.op, &p.args).await?;
            receipt(started)
        }
        "page.keyboard" => {
            #[derive(Deserialize)]
            struct KeyboardParams {
                op: String,
                #[serde(default)]
                args: Value,
            }
            let p: KeyboardParams = parse(params)?;
            let started = Instant::now();
            let sid = session(gw, page).await?;
            super::actions::raw_keyboard(gw, &sid, &p.op, &p.args).await?;
            receipt(started)
        }
        "page.fetch" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct FetchParams {
                url: String,
                #[serde(default)]
                options: Value,
                #[serde(default)]
                save_as: Option<String>,
            }
            let p: FetchParams = parse(params)?;
            if let Some(path) = &p.save_as
                && !std::path::Path::new(path).is_absolute()
            {
                return Err(RpcError::invalid_params(format!(
                    "saveAs 必须是绝对路径：{path}"
                )));
            }
            let sid = session(gw, page).await?;
            super::actions::fetch(gw, &sid, &p.url, &p.options, p.save_as.as_deref()).await
        }
        "page.close" => {
            let (id, label) = (page.id, page.label.clone());
            gw.op(move |t| t.page_close(id, &label)).await?;
            Ok(json!({}))
        }
        "page.cdp" => {
            #[derive(Deserialize)]
            struct CdpParams {
                method: String,
                #[serde(default)]
                params: Value,
            }
            let p: CdpParams = parse(params)?;
            // 先订阅接管信号再检查：检查通过之后、发送之前任何 TaskSpace 被接管，都中止
            let mut takeover = gw.takeover.subscribe_global();
            let id = page.id;
            gw.op(move |t| t.ensure_raw_cdp_allowed(id)).await?;
            let params = if p.params.is_null() {
                json!({})
            } else {
                p.params
            };
            // 接管信号随命令进入写出队列：写出之前接管了也不发
            let guard = takeover.clone();
            let send = async {
                let sid = session(gw, page).await?;
                Ok::<_, RpcError>(
                    gw.cdp
                        .send_guarded(Some(&sid), &p.method, params, guard)
                        .await?,
                )
            };
            // 建立页面会话的命令也带全局接管信号
            let send = crate::cdp::GUARD.scope(takeover.clone(), send);
            let result = tokio::select! {
                biased;
                _ = takeover.changed() => return Err(raw_cdp_interrupted()),
                r = send => r?,
            };
            Ok(json!({"result": result}))
        }
        "page.evaluate" => {
            #[derive(Deserialize)]
            struct EvalParams {
                #[serde(rename = "fn")]
                func: String,
                #[serde(default)]
                arg: Value,
            }
            let p: EvalParams = parse(params)?;
            let sid = session(gw, page).await?;
            let value = super::actions::evaluate(gw, &sid, &p.func, &p.arg).await?;
            Ok(json!({"value": value}))
        }
        _ => {
            let p: super::actions::ActionParams = parse(params)?;
            let started = Instant::now();
            let sid = session(gw, page).await?;
            // 之前的动作留下、没被领走的弹窗不算这次的
            let (id, label) = (page.id, page.label.clone());
            gw.op(move |t| t.take_popups(id, &label)).await?;
            let mut events = gw.cdp.subscribe();

            let refs = gw
                .refs
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&(page.id, page.label.clone()))
                .cloned()
                .unwrap_or_default();
            super::actions::act(gw, &sid, method, &p, &refs).await?;
            // 页面在动作里打开的窗口：事件先于动作的最后一个 CDP 响应到达
            let mut opened = 0;
            while let Ok(ev) = events.try_recv() {
                if ev.session_id.as_deref() == Some(sid.as_str()) && ev.method == "Page.windowOpen"
                {
                    opened += 1;
                }
            }
            let popups = collect_popups(gw, page, opened).await?;
            let mut r = receipt(started)?;
            r["popups"] = serde_json::to_value(popups).map_err(RpcError::internal)?;
            Ok(r)
        }
    }
}

/// 原始 CDP 透传在发送前遇到接管。
pub(super) fn raw_cdp_interrupted() -> RpcError {
    RpcError::new(
        ErrorCode::UserControl,
        "有 TaskSpace 被用户接管，原始 CDP 透传已中止",
    )
}

/// 该页面会话上正开着的对话框。
fn open_dialog(gw: &Gateway, sid: &str) -> Option<alter_core::protocol::DialogInfo> {
    gw.dialogs
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(sid)
        .cloned()
}

fn receipt(started: Instant) -> Result<Value, RpcError> {
    serde_json::to_value(Receipt {
        ok: true,
        duration_ms: started.elapsed().as_millis() as u64,
        popups: Vec::new(),
        dialog: None,
    })
    .map_err(RpcError::internal)
}

/// 页面在 CDP 里的会话；页面不存在回 E_PAGE_NOT_FOUND。视图刚建、还没上报 targetId，或 app 刚启动、
/// 还没连上 Chromium 时最多等 5 秒。
pub(super) async fn session(gw: &Gateway, page: &PageParams) -> Result<String, RpcError> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let (id, label) = (page.id, page.label.clone());
        let target = gw.op(move |t| t.page_target(id, &label)).await?;
        if let Some(target) = target
            && gw.cdp.connected().await
        {
            let sid = gw.cdp.session(&target).await?;
            allow_downloads(gw, &target, &sid).await?;
            return Ok(sid);
        }
        if Instant::now() >= deadline {
            return Err(RpcError::new(
                ErrorCode::Internal,
                "页面还没就绪（没有 CDP target，或还没连上 Chromium）",
            ));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// 页面的 url、title、是否在加载：连着 CDP 时在页面里读，否则用标签回写的值。
async fn info(gw: &Gateway, page: &PageParams) -> Result<Value, RpcError> {
    let (id, label) = (page.id, page.label.clone());
    let stored = gw.op(move |t| t.page_info(id, &label)).await?;
    let (id, label) = (page.id, page.label.clone());
    let target = gw.op(move |t| t.page_target(id, &label)).await?;
    let Some(target) = target else {
        return Ok(stored);
    };
    if !gw.cdp.connected().await {
        return Ok(stored);
    }
    let sid = gw.cdp.session(&target).await?;
    if let Some(d) = open_dialog(gw, &sid) {
        let mut stored = stored;
        stored["dialog"] = serde_json::to_value(d).map_err(RpcError::internal)?;
        return Ok(stored);
    }
    let r = gw
        .cdp
        .send(
            Some(&sid),
            "Runtime.evaluate",
            json!({
                "expression": "({url: location.href, title: document.title, loading: document.readyState !== 'complete'})",
                "returnByValue": true,
            }),
        )
        .await?;
    Ok(r["result"]["value"].clone())
}

/// 让页面所在的浏览器上下文（Space 的 partition）把下载存进下载目录（文件先以 guid 命名），
/// 每个上下文只设置一次。Electron 的 partition 不在 DevTools 的上下文表里，`Browser.setDownloadBehavior`
/// 按 browserContextId 找不到它，只能经页面会话的 `Page.setDownloadBehavior`（作用于页面所在的上下文）。
/// 下载事件随之以 `Page.downloadWillBegin` / `Page.downloadProgress` 发到开了 Page 域的会话。
async fn allow_downloads(gw: &Gateway, target: &str, sid: &str) -> Result<(), RpcError> {
    let info = gw
        .cdp
        .send(None, "Target.getTargetInfo", json!({"targetId": target}))
        .await?;
    let Some(context) = info["targetInfo"]["browserContextId"].as_str() else {
        return Ok(());
    };
    let mut done = gw.download_contexts.lock().await;
    if done.contains(context) {
        return Ok(());
    }
    std::fs::create_dir_all(&gw.download_dir)
        .map_err(|e| RpcError::internal(format!("创建下载目录失败：{e}")))?;
    gw.cdp
        .send(
            Some(sid),
            "Page.setDownloadBehavior",
            json!({
                "behavior": "allowAndName",
                "downloadPath": gw.download_dir.to_string_lossy(),
            }),
        )
        .await?;
    done.insert(context.to_owned());
    Ok(())
}

/// 动作期间页面打开了 `opened` 个窗口：等 TabService 把它们记成 Page、视图上报 targetId，再交给回执。最多等 5 秒，没等到的不放进回执（之后仍有 `page.popup` 事件）。
async fn collect_popups(
    gw: &Gateway,
    page: &PageParams,
    opened: usize,
) -> Result<Vec<Popup>, RpcError> {
    let mut popups = Vec::new();
    if opened == 0 {
        return Ok(popups);
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while popups.len() < opened && Instant::now() < deadline {
        let (id, label) = (page.id, page.label.clone());
        popups.extend(gw.op(move |t| t.take_popups(id, &label)).await?);
        if popups.len() < opened {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    Ok(popups)
}
