//! 接管信号：TabService 把某个 TaskSpace 的 ownership 写成 user 的同一步里递增（单线程运行时下不可分）。
//! - 每个 TaskSpace 一个：该 TaskSpace 正在执行、排队中的 agent 命令据此中止，入队未写出的 CDP 命令被丢弃；
//! - 全局一个：任何 TaskSpace 被接管都递增，原始 CDP 透传与 CDP 转发据此中止 / 断开（它们可作用于任何页面）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::watch;

pub struct Takeover {
    global: watch::Sender<u64>,
    per_task_space: Mutex<HashMap<i64, watch::Sender<u64>>>,
}

impl Takeover {
    pub fn new() -> Arc<Takeover> {
        Arc::new(Takeover {
            global: watch::channel(0).0,
            per_task_space: Mutex::new(HashMap::new()),
        })
    }

    /// 订阅全局接管信号（订阅之前的接管不算）。
    pub fn subscribe_global(&self) -> watch::Receiver<u64> {
        self.global.subscribe()
    }

    /// 订阅某个 TaskSpace 的接管信号（订阅之前的接管不算）。
    pub fn subscribe(&self, task_space_id: i64) -> watch::Receiver<u64> {
        self.per_task_space
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(task_space_id)
            .or_insert_with(|| watch::channel(0).0)
            .subscribe()
    }

    /// 某个 TaskSpace 被用户接管：递增它的信号与全局信号。
    pub fn fire(&self, task_space_id: i64) {
        if let Some(tx) = self
            .per_task_space
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&task_space_id)
        {
            tx.send_modify(|n| *n += 1);
        }
        self.global.send_modify(|n| *n += 1);
    }
}
