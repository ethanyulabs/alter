//! 自动归档：周期 `min(阈值 / 3, 60 s)`，让 TabService 把超过阈值没激活过的标签归档，有变化才广播。

use std::time::Duration;

use tokio::sync::mpsc;

use crate::lifecycle::now_millis;
use crate::tabs::{GatewayJob, Request};

pub async fn run(requests: mpsc::UnboundedSender<Request>, threshold: Duration) {
    let period = (threshold / 3).clamp(Duration::from_millis(10), Duration::from_secs(60));
    let threshold_ms = threshold.as_millis() as i64;
    let mut tick = tokio::time::interval(period);
    loop {
        tick.tick().await;
        let job: GatewayJob = Box::new(move |t| t.archive_stale(now_millis(), threshold_ms));
        if requests.send(Request::Gateway(job)).is_err() {
            return;
        }
    }
}
