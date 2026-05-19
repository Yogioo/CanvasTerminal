/// 定时器管理模块
///
/// 管理每个脚本节点的定时器状态，提供 poll/set/clear 接口。
/// 与 Lua 的 on_tick 配合使用，定时驱动帧刷新。

use std::collections::HashMap;
use std::time::Instant;

/// 定时器管理器
///
/// 维护每个节点 ID → 定时器间隔的映射。
/// 通过 poll 方法检查定时器是否到期。
#[derive(Debug, Clone)]
pub struct TimerManager {
    /// 节点 ID → 间隔（秒），0.0 = 未激活
    intervals: HashMap<usize, f64>,
    /// 节点 ID → 上次 tick 时间
    last_tick: HashMap<usize, Instant>,
}

impl Default for TimerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TimerManager {
    /// 创建新的定时器管理器
    pub fn new() -> Self {
        TimerManager {
            intervals: HashMap::new(),
            last_tick: HashMap::new(),
        }
    }

    /// 轮询指定节点的定时器是否到期
    ///
    /// 如果定时器激活且距上次 tick 超过间隔，返回 Some(dt)：
    /// - dt = 距上次 tick 的实际经过时间（秒）
    /// - 同时更新 last_tick 为当前时间
    ///
    /// 如果定时器未激活或未到期，返回 None。
    ///
    /// # 参数
    ///
    /// * `node_id` - 节点 ID
    /// * `now` - 当前时间
    pub fn poll(&mut self, node_id: usize, now: Instant) -> Option<f64> {
        let interval = self.intervals.get(&node_id)?;
        if *interval <= 0.0 {
            return None;
        }
        let last = self.last_tick.entry(node_id).or_insert(now);
        let elapsed = (now - *last).as_secs_f64();
        if elapsed >= *interval {
            *last = now;
            Some(elapsed)
        } else {
            None
        }
    }

    /// 设置定时器间隔
    ///
    /// # 参数
    ///
    /// * `node_id` - 节点 ID
    /// * `interval` - 间隔秒数。0.0 = 停止定时器
    pub fn set(&mut self, node_id: usize, interval: f64) {
        self.intervals.insert(node_id, interval);
        if interval > 0.0 {
            self.last_tick.entry(node_id).or_insert_with(Instant::now);
        } else {
            self.last_tick.remove(&node_id);
        }
    }

    /// 清除（停止）指定节点的定时器
    pub fn clear(&mut self, node_id: usize) {
        self.intervals.remove(&node_id);
        self.last_tick.remove(&node_id);
    }

    /// 获取指定节点的定时器间隔
    ///
    /// 返回 0.0 表示未激活
    pub fn get_interval(&self, node_id: usize) -> f64 {
        self.intervals.get(&node_id).copied().unwrap_or(0.0)
    }

    /// 检查指定节点是否有激活的定时器
    pub fn has_active_timer(&self, node_id: usize) -> bool {
        self.intervals.get(&node_id).map_or(false, |&i| i > 0.0)
    }

    /// 检查是否有任何节点有激活的定时器
    pub fn any_active(&self) -> bool {
        self.intervals.values().any(|&i| i > 0.0)
    }

    /// 重置指定节点的定时器计时（不改变间隔）
    pub fn reset_tick(&mut self, node_id: usize) {
        if self.intervals.contains_key(&node_id) {
            self.last_tick.insert(node_id, Instant::now());
        }
    }

    /// 移除指定节点的所有定时器状态
    pub fn remove_node(&mut self, node_id: usize) {
        self.intervals.remove(&node_id);
        self.last_tick.remove(&node_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_timer_no_active() {
        let mut tm = TimerManager::new();
        assert!(!tm.has_active_timer(1));
        assert!(!tm.any_active());
    }

    #[test]
    fn test_timer_set_and_poll() {
        let mut tm = TimerManager::new();
        tm.set(1, 0.1);
        assert!(tm.has_active_timer(1));
        assert!(tm.any_active());
        assert!((tm.get_interval(1) - 0.1).abs() < 0.001);

        // 立即 poll 不应触发（未到时间）
        let now = Instant::now();
        let result = tm.poll(1, now);
        assert!(result.is_none());
    }

    #[test]
    fn test_timer_clear() {
        let mut tm = TimerManager::new();
        tm.set(1, 1.0);
        assert!(tm.has_active_timer(1));
        tm.clear(1);
        assert!(!tm.has_active_timer(1));
    }

    #[test]
    fn test_multiple_nodes() {
        let mut tm = TimerManager::new();
        tm.set(1, 1.0);
        tm.set(2, 2.0);
        assert!(tm.has_active_timer(1));
        assert!(tm.has_active_timer(2));
        assert!(tm.any_active());
        tm.remove_node(1);
        assert!(!tm.has_active_timer(1));
        assert!(tm.has_active_timer(2));
    }
}
