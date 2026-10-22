//! 页面上的元素动作、无障碍快照与 evaluate：都经 CdpMux 对页面会话发 CDP。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use alter_core::protocol::ErrorCode;
use serde::Deserialize;
use serde_json::{Value, json};

use super::Gateway;
use super::rpc::RpcError;

/// 可交互的无障碍角色：快照里给它们 `@e<n>` 引用。
const INTERACTIVE: &[&str] = &[
    "button",
    "link",
    "textbox",
    "searchbox",
    "checkbox",
    "radio",
    "combobox",
    "menuitem",
    "tab",
    "option",
    "switch",
    "slider",
    "spinbutton",
];

/// 快照里不单独成行的节点（只是排版用的）。
const SKIP: &[&str] = &["InlineTextBox", "LineBreak", "none", "generic"];

/// 页面上一次 snapshot 生成的 `@e<n>` → backendDOMNodeId。
pub(super) type Refs = HashMap<String, i64>;

/// `page.snapshot`：返回文本与引用数，同时把引用存进 `refs`。`root` 为某个 DOM 节点的
/// backendNodeId 时只输出它在无障碍树里的子树。
pub(super) async fn snapshot(
    gw: &Gateway,
    sid: &str,
    root: Option<i64>,
) -> Result<(String, Refs), RpcError> {
    let tree = gw
        .cdp
        .send(Some(sid), "Accessibility.getFullAXTree", json!({}))
        .await?;
    let nodes = tree["nodes"].as_array().cloned().unwrap_or_default();
    let by_id: HashMap<&str, &Value> = nodes
        .iter()
        .filter_map(|n| n["nodeId"].as_str().map(|id| (id, n)))
        .collect();
    let root = match root {
        None => nodes.iter().find(|n| n.get("parentId").is_none()),
        Some(backend) => Some(
            nodes
                .iter()
                .find(|n| n["backendDOMNodeId"].as_i64() == Some(backend))
                .ok_or_else(|| {
                    RpcError::new(ErrorCode::NoMatch, "root 元素在无障碍树里没有节点")
                })?,
        ),
    };
    let mut out = String::new();
    let mut refs = Refs::new();
    if let Some(root) = root {
        walk(root, &by_id, 0, &mut out, &mut refs);
    }
    Ok((out, refs))
}

fn walk(
    node: &Value,
    by_id: &HashMap<&str, &Value>,
    depth: usize,
    out: &mut String,
    refs: &mut Refs,
) {
    let role = node["role"]["value"].as_str().unwrap_or("");
    let ignored = node["ignored"].as_bool().unwrap_or(false);
    let shown = !ignored && !SKIP.contains(&role);
    let next_depth = if shown {
        // 与无障碍名称的规范化一致：去掉首尾空白，连续空白合成一个空格
        let name = node["name"]["value"]
            .as_str()
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&"  ".repeat(depth));
        out.push_str(&format!(
            "- {role} {}",
            serde_json::to_string(&name).unwrap_or_default()
        ));
        if INTERACTIVE.contains(&role)
            && let Some(backend) = node["backendDOMNodeId"].as_i64()
        {
            let r = format!("@e{}", refs.len() + 1);
            out.push_str(&format!(" [{r}]"));
            refs.insert(r, backend);
        }
        out.push('\n');
        depth + 1
    } else {
        depth
    };
    for child in node["childIds"].as_array().into_iter().flatten() {
        if let Some(c) = child.as_str().and_then(|id| by_id.get(id)) {
            walk(c, by_id, next_depth, out, refs);
        }
    }
}

/// 在页面里找元素的函数：返回匹配元素的数组。`text=` 取文字恰好相等、且没有同样匹配的后代的元素。
const FIND_JS: &str = r#"(kind, arg) => {
  if (kind === "css") return Array.from(document.querySelectorAll(arg));
  if (kind === "xpath") {
    const r = document.evaluate(arg, document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
    return Array.from({ length: r.snapshotLength }, (_, i) => r.snapshotItem(i));
  }
  const all = Array.from(document.querySelectorAll("body *")).filter(
    (el) => (el.innerText ?? el.textContent ?? "").trim() === arg);
  return all.filter((el) => !all.some((o) => o !== el && el.contains(o)));
}"#;

/// 解析后的选择器。
enum Selector {
    Ref(String),
    Role { role: String, name: Option<String> },
    Js { kind: &'static str, arg: String },
}

fn parse_selector(s: &str) -> Result<Selector, RpcError> {
    if s.starts_with("@e") {
        return Ok(Selector::Ref(s.to_owned()));
    }
    if let Some(rest) = s.strip_prefix("loc=role:") {
        // role[name="…"]
        let (role, name) = match rest.split_once('[') {
            Some((role, attr)) => {
                let attr = attr.trim_end_matches(']');
                let name = attr
                    .strip_prefix("name=")
                    .map(|v| v.trim_matches('"').to_owned())
                    .ok_or_else(|| {
                        RpcError::invalid_params(format!("不支持的 role 选择器：{s}"))
                    })?;
                (role.to_owned(), Some(name))
            }
            None => (rest.to_owned(), None),
        };
        return Ok(Selector::Role { role, name });
    }
    if let Some(t) = s.strip_prefix("text=") {
        return Ok(Selector::Js {
            kind: "text",
            arg: t.to_owned(),
        });
    }
    if let Some(c) = s.strip_prefix("loc=css:") {
        return Ok(Selector::Js {
            kind: "css",
            arg: c.to_owned(),
        });
    }
    if let Some(x) = s.strip_prefix("xpath=") {
        return Ok(Selector::Js {
            kind: "xpath",
            arg: x.to_owned(),
        });
    }
    Ok(Selector::Js {
        kind: "css",
        arg: s.to_owned(),
    })
}

/// 选择器当前匹配到的元素（objectId）。
async fn matches(
    gw: &Gateway,
    sid: &str,
    sel: &Selector,
    refs: &Refs,
) -> Result<Vec<String>, RpcError> {
    match sel {
        Selector::Ref(r) => {
            let Some(backend) = refs.get(r) else {
                return Ok(Vec::new());
            };
            match gw
                .cdp
                .send(
                    Some(sid),
                    "DOM.resolveNode",
                    json!({"backendNodeId": backend}),
                )
                .await
            {
                Ok(v) => Ok(v["object"]["objectId"]
                    .as_str()
                    .map(str::to_owned)
                    .into_iter()
                    .collect()),
                // 节点已不在页面里
                Err(_) => Ok(Vec::new()),
            }
        }
        Selector::Role { role, name } => {
            let doc = gw
                .cdp
                .send(
                    Some(sid),
                    "Runtime.evaluate",
                    json!({"expression": "document"}),
                )
                .await?;
            let mut q = json!({"objectId": doc["result"]["objectId"], "role": role});
            if let Some(n) = name {
                q["accessibleName"] = json!(n);
            }
            let r = gw
                .cdp
                .send(Some(sid), "Accessibility.queryAXTree", q)
                .await?;
            let mut out = Vec::new();
            for n in r["nodes"].as_array().into_iter().flatten() {
                if n["ignored"].as_bool().unwrap_or(false) {
                    continue;
                }
                if let Some(b) = n["backendDOMNodeId"].as_i64() {
                    let v = gw
                        .cdp
                        .send(Some(sid), "DOM.resolveNode", json!({"backendNodeId": b}))
                        .await?;
                    if let Some(id) = v["object"]["objectId"].as_str() {
                        out.push(id.to_owned());
                    }
                }
            }
            Ok(out)
        }
        Selector::Js { kind, arg } => {
            let expr = format!("({FIND_JS})({}, {})", json!(kind), json!(arg));
            let arr = gw
                .cdp
                .send(Some(sid), "Runtime.evaluate", json!({"expression": expr}))
                .await?;
            let Some(array_id) = arr["result"]["objectId"].as_str() else {
                return Ok(Vec::new());
            };
            let props = gw
                .cdp
                .send(
                    Some(sid),
                    "Runtime.getProperties",
                    json!({"objectId": array_id, "ownProperties": true}),
                )
                .await?;
            Ok(props["result"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|p| {
                    p["name"]
                        .as_str()
                        .is_some_and(|n| n.parse::<usize>().is_ok())
                })
                .filter_map(|p| p["value"]["objectId"].as_str().map(str::to_owned))
                .collect())
        }
    }
}

/// 在 `timeout` 内等选择器恰好匹配一个元素：多于一个立即 E_AMBIGUOUS_SELECTOR，超时 E_NO_MATCH。
pub(super) async fn locate(
    gw: &Gateway,
    sid: &str,
    selector: &str,
    refs: &Refs,
    timeout: Duration,
) -> Result<String, RpcError> {
    let sel = parse_selector(selector)?;
    let deadline = Instant::now() + timeout;
    loop {
        let found = matches(gw, sid, &sel, refs).await?;
        match found.len() {
            1 => return Ok(found.into_iter().next().unwrap_or_default()),
            0 => {}
            n => {
                return Err(RpcError::with_details(
                    ErrorCode::AmbiguousSelector,
                    format!("选择器 {selector} 匹配了 {n} 个元素"),
                    json!({"count": n}),
                ));
            }
        }
        if Instant::now() >= deadline {
            return Err(RpcError::new(
                ErrorCode::NoMatch,
                format!("{} ms 内没有元素匹配 {selector}", timeout.as_millis()),
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// 元素的中心点（先滚到可见区域）。
async fn center(gw: &Gateway, sid: &str, object: &str) -> Result<(f64, f64), RpcError> {
    let _ = gw
        .cdp
        .send(
            Some(sid),
            "DOM.scrollIntoViewIfNeeded",
            json!({"objectId": object}),
        )
        .await;
    let q = gw
        .cdp
        .send(
            Some(sid),
            "DOM.getContentQuads",
            json!({"objectId": object}),
        )
        .await?;
    let quad: Vec<f64> = q["quads"][0]
        .as_array()
        .ok_or_else(|| RpcError::new(ErrorCode::NoMatch, "元素不可见（没有内容区域）"))?
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    let xs = [quad[0], quad[2], quad[4], quad[6]];
    let ys = [quad[1], quad[3], quad[5], quad[7]];
    Ok((xs.iter().sum::<f64>() / 4.0, ys.iter().sum::<f64>() / 4.0))
}

async fn mouse(
    gw: &Gateway,
    sid: &str,
    kind: &str,
    x: f64,
    y: f64,
    clicks: u32,
) -> Result<(), RpcError> {
    let mut p = json!({"type": kind, "x": x, "y": y});
    if kind != "mouseMoved" {
        p["button"] = json!("left");
        p["clickCount"] = json!(clicks);
    }
    gw.cdp
        .send(Some(sid), "Input.dispatchMouseEvent", p)
        .await?;
    Ok(())
}

async fn call_on(
    gw: &Gateway,
    sid: &str,
    object: &str,
    func: &str,
    args: Value,
) -> Result<Value, RpcError> {
    let r = gw
        .cdp
        .send(
            Some(sid),
            "Runtime.callFunctionOn",
            json!({"objectId": object, "functionDeclaration": func, "arguments": args, "returnByValue": true, "awaitPromise": true}),
        )
        .await?;
    Ok(r["result"]["value"].clone())
}

/// 按键名到 CDP 键事件参数。
fn key_params(key: &str) -> Value {
    match key {
        "Enter" => {
            json!({"key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13, "text": "\r"})
        }
        "Tab" => json!({"key": "Tab", "code": "Tab", "windowsVirtualKeyCode": 9}),
        "Escape" => json!({"key": "Escape", "code": "Escape", "windowsVirtualKeyCode": 27}),
        "Backspace" => json!({"key": "Backspace", "code": "Backspace", "windowsVirtualKeyCode": 8}),
        "ArrowDown" => {
            json!({"key": "ArrowDown", "code": "ArrowDown", "windowsVirtualKeyCode": 40})
        }
        "ArrowUp" => json!({"key": "ArrowUp", "code": "ArrowUp", "windowsVirtualKeyCode": 38}),
        k => json!({"key": k, "text": k}),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ActionParams {
    pub selector: String,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// 3–6 个词的动作描述：有它时先在页面上显示 agent 光标与这段文字
    #[serde(default)]
    pub action_label: Option<String>,
}

/// 执行一个元素动作。
pub(super) async fn act(
    gw: &Gateway,
    sid: &str,
    method: &str,
    p: &ActionParams,
    refs: &Refs,
) -> Result<(), RpcError> {
    let timeout = Duration::from_millis(p.timeout_ms.unwrap_or(5_000));
    let el = locate(gw, sid, &p.selector, refs, timeout).await?;
    if let Some(text) = p.action_label.as_deref().filter(|t| !t.is_empty()) {
        // 叠层只是提示：取不到位置或注入失败都不影响动作本身
        if let Ok((x, y)) = center(gw, sid, &el).await
            && let Some(target) = gw.cdp.target_of(sid).await
            && let Err(e) = crate::cdp::overlay::show(&gw.cdp, sid, &target, x, y, text).await
        {
            tracing::warn!(error = %e, "显示 agent 叠层失败");
        }
    }
    let value_str = || {
        p.value
            .as_ref()
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    match method {
        "page.click" | "page.dblclick" => {
            let (x, y) = center(gw, sid, &el).await?;
            mouse(gw, sid, "mouseMoved", x, y, 0).await?;
            let clicks = if method == "page.dblclick" { 2 } else { 1 };
            for n in 1..=clicks {
                mouse(gw, sid, "mousePressed", x, y, n).await?;
                mouse(gw, sid, "mouseReleased", x, y, n).await?;
            }
        }
        "page.hover" => {
            let (x, y) = center(gw, sid, &el).await?;
            mouse(gw, sid, "mouseMoved", x, y, 0).await?;
        }
        "page.focus" => {
            call_on(gw, sid, &el, "function(){ this.focus(); }", json!([])).await?;
        }
        "page.fill" => {
            call_on(
                gw,
                sid,
                &el,
                "function(){ this.focus(); if ('value' in this) { this.value = ''; this.dispatchEvent(new Event('input', {bubbles: true})); } }",
                json!([]),
            )
            .await?;
            gw.cdp
                .send(Some(sid), "Input.insertText", json!({"text": value_str()}))
                .await?;
        }
        "page.press" => {
            call_on(gw, sid, &el, "function(){ this.focus(); }", json!([])).await?;
            let mut down = key_params(&value_str());
            down["type"] = json!("keyDown");
            gw.cdp
                .send(Some(sid), "Input.dispatchKeyEvent", down.clone())
                .await?;
            down["type"] = json!("keyUp");
            down.as_object_mut().map(|o| o.remove("text"));
            gw.cdp
                .send(Some(sid), "Input.dispatchKeyEvent", down)
                .await?;
        }
        "page.selectOption" => {
            let values = match &p.value {
                Some(Value::Array(a)) => Value::Array(a.clone()),
                Some(v) => json!([v]),
                None => json!([]),
            };
            call_on(
                gw,
                sid,
                &el,
                "function(vals){ for (const o of this.options) o.selected = vals.includes(o.value) || vals.includes(o.label); this.dispatchEvent(new Event('input', {bubbles: true})); this.dispatchEvent(new Event('change', {bubbles: true})); }",
                json!([{"value": values}]),
            )
            .await?;
        }
        "page.dragAndDrop" => {
            let target = locate(gw, sid, &value_str(), refs, timeout).await?;
            let (x1, y1) = center(gw, sid, &el).await?;
            let (x2, y2) = center(gw, sid, &target).await?;
            mouse(gw, sid, "mouseMoved", x1, y1, 0).await?;
            mouse(gw, sid, "mousePressed", x1, y1, 1).await?;
            mouse(gw, sid, "mouseMoved", (x1 + x2) / 2.0, (y1 + y2) / 2.0, 0).await?;
            mouse(gw, sid, "mouseMoved", x2, y2, 0).await?;
            mouse(gw, sid, "mouseReleased", x2, y2, 1).await?;
        }
        "page.setInputFiles" => {
            let files = match &p.value {
                Some(Value::Array(a)) => Value::Array(a.clone()),
                Some(v) => json!([v]),
                None => json!([]),
            };
            gw.cdp
                .send(
                    Some(sid),
                    "DOM.setFileInputFiles",
                    json!({"files": files, "objectId": el}),
                )
                .await?;
        }
        _ => unreachable!("rpc 只把元素动作交到这里"),
    }
    Ok(())
}

/// `page.mouse`：原始指针事件。`op` 为 move / down / up / click / wheel，`args` 带 x、y，
/// 以及可选的 button（默认 left）、clickCount（默认 1）、deltaX / deltaY（wheel）。
pub(super) async fn raw_mouse(
    gw: &Gateway,
    sid: &str,
    op: &str,
    args: &Value,
) -> Result<(), RpcError> {
    let x = args["x"].as_f64().unwrap_or(0.0);
    let y = args["y"].as_f64().unwrap_or(0.0);
    let button = args["button"].as_str().unwrap_or("left");
    let clicks = args["clickCount"].as_u64().unwrap_or(1);
    let event = |kind: &str| {
        let mut p = json!({"type": kind, "x": x, "y": y});
        if kind == "mousePressed" || kind == "mouseReleased" {
            p["button"] = json!(button);
            p["clickCount"] = json!(clicks);
        }
        p
    };
    let kinds: &[&str] = match op {
        "move" => &["mouseMoved"],
        "down" => &["mousePressed"],
        "up" => &["mouseReleased"],
        "click" => &["mouseMoved", "mousePressed", "mouseReleased"],
        "wheel" => {
            let p = json!({
                "type": "mouseWheel", "x": x, "y": y,
                "deltaX": args["deltaX"].as_f64().unwrap_or(0.0),
                "deltaY": args["deltaY"].as_f64().unwrap_or(0.0),
            });
            gw.cdp
                .send(Some(sid), "Input.dispatchMouseEvent", p)
                .await?;
            return Ok(());
        }
        other => {
            return Err(RpcError::invalid_params(format!(
                "不支持的鼠标操作：{other}"
            )));
        }
    };
    for kind in kinds {
        gw.cdp
            .send(Some(sid), "Input.dispatchMouseEvent", event(kind))
            .await?;
    }
    Ok(())
}

/// `page.keyboard`：原始键盘事件。`op` 为 down / up / press（`args.key`）或 type（`args.text`，整段插入）。
pub(super) async fn raw_keyboard(
    gw: &Gateway,
    sid: &str,
    op: &str,
    args: &Value,
) -> Result<(), RpcError> {
    if op == "type" {
        let text = args["text"].as_str().unwrap_or("");
        gw.cdp
            .send(Some(sid), "Input.insertText", json!({"text": text}))
            .await?;
        return Ok(());
    }
    let key = args["key"].as_str().unwrap_or("");
    let kinds: &[&str] = match op {
        "down" => &["keyDown"],
        "up" => &["keyUp"],
        "press" => &["keyDown", "keyUp"],
        other => {
            return Err(RpcError::invalid_params(format!(
                "不支持的键盘操作：{other}"
            )));
        }
    };
    for kind in kinds {
        let mut p = key_params(key);
        p["type"] = json!(kind);
        if *kind == "keyUp" {
            p.as_object_mut().map(|o| o.remove("text"));
        }
        gw.cdp.send(Some(sid), "Input.dispatchKeyEvent", p).await?;
    }
    Ok(())
}

/// 在页面里执行 fetch 的函数：带 Cookie 与页面的来源；`binary` 为真时正文以 base64 返回。
const FETCH_JS: &str = r#"async (url, options, binary) => {
  const r = await fetch(url, options ?? {});
  const headers = {};
  r.headers.forEach((v, k) => { headers[k] = v; });
  if (!binary) return { ok: r.ok, status: r.status, headers, body: await r.text() };
  const bytes = new Uint8Array(await r.arrayBuffer());
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return { ok: r.ok, status: r.status, headers, base64: btoa(s) };
}"#;

/// `page.fetch`：返回 `{ok, status, headers, body}`；有 `save_as`（绝对路径）时把正文写进文件，返回 `path` 代替 `body`。
pub(super) async fn fetch(
    gw: &Gateway,
    sid: &str,
    url: &str,
    options: &Value,
    save_as: Option<&str>,
) -> Result<Value, RpcError> {
    let expr = format!(
        "({FETCH_JS})({}, {}, {})",
        json!(url),
        options,
        save_as.is_some()
    );
    let r = gw
        .cdp
        .send(
            Some(sid),
            "Runtime.evaluate",
            json!({"expression": expr, "returnByValue": true, "awaitPromise": true}),
        )
        .await?;
    if let Some(ex) = r.get("exceptionDetails") {
        let msg = ex["exception"]["description"]
            .as_str()
            .or_else(|| ex["text"].as_str())
            .unwrap_or("fetch 失败");
        return Err(RpcError::new(ErrorCode::Internal, msg.to_owned()));
    }
    let mut v = r["result"]["value"].clone();
    if let Some(path) = save_as {
        let bytes = base64_decode(v["base64"].as_str().unwrap_or(""))
            .ok_or_else(|| RpcError::internal("fetch 返回的正文不是合法的 base64"))?;
        std::fs::write(path, bytes)
            .map_err(|e| RpcError::new(ErrorCode::InvalidArgument, format!("写文件失败：{e}")))?;
        if let Some(o) = v.as_object_mut() {
            o.remove("base64");
            o.insert("path".into(), json!(path));
        }
    }
    Ok(v)
}

/// `page.evaluate`：在页面里执行函数源码（带一个参数），返回可序列化的结果。
pub(super) async fn evaluate(
    gw: &Gateway,
    sid: &str,
    func: &str,
    arg: &Value,
) -> Result<Value, RpcError> {
    let expr = format!("({func})({arg})");
    let r = gw
        .cdp
        .send(
            Some(sid),
            "Runtime.evaluate",
            json!({"expression": expr, "returnByValue": true, "awaitPromise": true}),
        )
        .await?;
    if let Some(ex) = r.get("exceptionDetails") {
        let msg = ex["exception"]["description"]
            .as_str()
            .or_else(|| ex["text"].as_str())
            .unwrap_or("页面里抛出了异常");
        return Err(RpcError::new(ErrorCode::Internal, msg.to_owned()));
    }
    Ok(r["result"]["value"].clone())
}

/// `page.waitFor`：每 100 ms 检查一次，`timeout` 内满足即返回，否则 E_TIMEOUT。
pub(super) async fn wait_for(
    gw: &Gateway,
    sid: &str,
    kind: &str,
    arg: &Value,
    timeout: Duration,
) -> Result<(), RpcError> {
    if kind == "ms" {
        let ms = arg.as_u64().unwrap_or(0);
        if Duration::from_millis(ms) > timeout {
            return Err(RpcError::new(ErrorCode::Timeout, "等待时间超过 timeoutMs"));
        }
        tokio::time::sleep(Duration::from_millis(ms)).await;
        return Ok(());
    }
    let expr = match kind {
        "url" => format!("location.href.includes({arg})"),
        "selector" => format!("!!document.querySelector({arg})"),
        // 先等函数的结果（异步函数返回的是 Promise），再判真假
        "fn" => format!(
            "(async () => !!(await ({})()))()",
            arg.as_str().unwrap_or("() => false")
        ),
        other => {
            return Err(RpcError::invalid_params(format!(
                "不支持的等待条件：{other}"
            )));
        }
    };
    let deadline = Instant::now() + timeout;
    let timed_out = || {
        RpcError::new(
            ErrorCode::Timeout,
            format!("{} ms 内没有等到 {kind} {arg}", timeout.as_millis()),
        )
    };
    loop {
        // 每次求值也受剩余期限约束：函数可能返回永不完成的 Promise
        let left = deadline.saturating_duration_since(Instant::now());
        let r = tokio::time::timeout(
            left,
            gw.cdp.send(
                Some(sid),
                "Runtime.evaluate",
                json!({"expression": expr, "returnByValue": true, "awaitPromise": true}),
            ),
        )
        .await
        .map_err(|_| timed_out())??;
        if r["result"]["value"].as_bool() == Some(true) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(RpcError::new(
                ErrorCode::Timeout,
                format!("{} ms 内没有等到 {kind} {arg}", timeout.as_millis()),
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// 标准 base64 解码（截图数据）；非法输入返回 None。
pub(super) fn base64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    }
    let bytes: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let pad = chunk.iter().filter(|&&b| b == b'=').count();
        let mut n = 0u32;
        for (i, &b) in chunk.iter().enumerate() {
            let v = if b == b'=' { 0 } else { val(b)? };
            n |= v << (18 - 6 * i as u32);
        }
        let take = 3usize
            .saturating_sub(pad)
            .min(chunk.len().saturating_sub(1));
        out.extend_from_slice(&n.to_be_bytes()[1..1 + take]);
    }
    Some(out)
}

/// PNG 的宽高（IHDR，第 16–24 字节）。
pub(super) fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    if png.len() < 24 || png[..4] != [0x89, b'P', b'N', b'G'] {
        return None;
    }
    let w = u32::from_be_bytes(png[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(png[20..24].try_into().ok()?);
    Some((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("aGk=").unwrap(), b"hi");
        assert_eq!(base64_decode("YWJj").unwrap(), b"abc");
        assert!(base64_decode("@@@@").is_none());
    }
}
