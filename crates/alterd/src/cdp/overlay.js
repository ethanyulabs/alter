/* global document */
/* eslint-disable @typescript-eslint/no-unused-vars -- 这些函数由 overlay.rs 拼接在脚本后面的调用使用 */
// agent 叠层：在页面上画 agent 光标与动作文字。在隔离 world 里执行，页面脚本拿不到这里的变量。
// 元素 <alter-agent-overlay> 挂在 documentElement 下（各 world 共享 DOM），状态记在它的属性上：
// 同名隔离 world 可能每次都是新的执行上下文，这里不依赖任何全局变量。
// 隔离 world 里没有 customElements，所以只用合法的自定义元素名 + open shadow root，不注册自定义元素。
// 用法：在本脚本之后调用 __alterOverlayShow(x, y, text) / __alterOverlayHide() / __alterOverlaySuspend() / __alterOverlayResume()。
var __alterOverlayHost = function () {
  var host = document.querySelector("alter-agent-overlay");
  if (host && host.shadowRoot) return host;
  host = document.createElement("alter-agent-overlay");
  host.style.cssText = "position:fixed;inset:0;pointer-events:none;z-index:2147483647;display:none;";
  var root = host.attachShadow({ mode: "open" });
  root.innerHTML =
    "<style>" +
    "[data-role=cursor]{position:fixed;width:16px;height:16px;margin:-8px 0 0 -8px;border-radius:50%;" +
    "background:rgba(59,130,246,.85);box-shadow:0 0 0 3px rgba(59,130,246,.35);}" +
    "[data-role=action]{position:fixed;padding:2px 8px;border-radius:6px;background:rgba(17,24,39,.85);" +
    "color:#fff;font:12px/1.6 -apple-system,sans-serif;white-space:nowrap;}" +
    "</style><div data-role=\"cursor\"></div><div data-role=\"action\"></div>";
  document.documentElement.appendChild(host);
  return host;
};
var __alterOverlayShow = function (x, y, text) {
  var host = __alterOverlayHost();
  var root = host.shadowRoot;
  var cursor = root.querySelector("[data-role=cursor]");
  var action = root.querySelector("[data-role=action]");
  cursor.style.left = x + "px";
  cursor.style.top = y + "px";
  action.textContent = text;
  action.style.left = x + 14 + "px";
  action.style.top = y + 10 + "px";
  host.setAttribute("data-visible", "true");
  host.style.display = "block";
};
var __alterOverlayHide = function () {
  var host = document.querySelector("alter-agent-overlay");
  if (!host) return;
  host.setAttribute("data-visible", "false");
  host.style.display = "none";
};
// 截图前暂时藏起，截完按原来的可见状态恢复
var __alterOverlaySuspend = function () {
  var host = document.querySelector("alter-agent-overlay");
  if (host) host.style.display = "none";
};
var __alterOverlayResume = function () {
  var host = document.querySelector("alter-agent-overlay");
  if (host && host.getAttribute("data-visible") === "true") host.style.display = "block";
};
