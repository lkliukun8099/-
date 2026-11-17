//! 熔断器状态机（N-06：触发后 ≤1s 切换备用链）。
//!
//! 状态转移：Closed --连续3次失败/超时--> Open(冷却30s) --到期--> HalfOpen(单探测)
//! HalfOpen --成功--> Closed；HalfOpen --失败--> Open(重新冷却)。

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// 熔断状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open { until: Instant },
    HalfOpen,
}

/// 单 `provider.model` 一个熔断器。
pub struct CircuitBreaker {
    state: Mutex<CircuitState>,
    consecutive_failures: AtomicU32,
    /// 连续失败阈值
    failure_threshold: u32,
    /// Open 冷却时长
    cooldown: Duration,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(CircuitState::Closed),
            consecutive_failures: AtomicU32::new(0),
            failure_threshold: 3,
            cooldown: Duration::from_secs(30),
        }
    }

    /// 当前是否放行请求。
    pub fn allow(&self) -> bool {
        let mut state = self.state.lock();
        match *state {
            CircuitState::Closed => true,
            CircuitState::Open { until } => {
                if Instant::now() >= until {
                    // 冷却到期，进入半开（放行单探测）
                    *state = CircuitState::HalfOpen;
                    true
                } else {
                    false
                }
            }
            // 半开状态只允许探测请求；并发下近似放行，由 on_* 收敛
            CircuitState::HalfOpen => true,
        }
    }

    /// 调用成功：复位。
    pub fn on_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        let mut state = self.state.lock();
        if *state != CircuitState::Closed {
            tracing::info!("熔断器恢复 Closed");
        }
        *state = CircuitState::Closed;
    }

    /// 调用失败：累计，达阈值进入 Open。
    pub fn on_failure(&self) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
        let mut state = self.state.lock();
        if *state == CircuitState::HalfOpen || failures >= self.failure_threshold {
            *state = CircuitState::Open {
                until: Instant::now() + self.cooldown,
            };
            tracing::warn!("熔断器进入 Open（连续失败 {failures} 次），冷却 {}s",
                self.cooldown.as_secs());
        }
    }

    /// 当前状态（监控/调试用）。
    pub fn state(&self) -> CircuitState {
        *self.state.lock()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breaker_trips_after_threshold() {
        let cb = CircuitBreaker::new();
        assert!(cb.allow());
        cb.on_failure();
        cb.on_failure();
        assert!(cb.allow(), "未达阈值仍放行");
        cb.on_failure();
        assert!(!cb.allow(), "连续 3 次失败应熔断");
        cb.on_success();
        assert!(cb.allow(), "成功后复位");
    }

    /// 构造自定义参数的熔断器（测试专用，缩短冷却避免等待 30s）。
    fn breaker_with(cooldown_ms: u64) -> CircuitBreaker {
        CircuitBreaker {
            state: Mutex::new(CircuitState::Closed),
            consecutive_failures: AtomicU32::new(0),
            failure_threshold: 3,
            cooldown: Duration::from_millis(cooldown_ms),
        }
    }

    #[test]
    fn open_blocks_until_cooldown_then_half_open() {
        let cb = breaker_with(30);
        cb.on_failure();
        cb.on_failure();
        cb.on_failure();
        assert!(matches!(cb.state(), CircuitState::Open { .. }), "3 连败应进入 Open");
        assert!(!cb.allow(), "Open 冷却期内不放行");
        std::thread::sleep(Duration::from_millis(40));
        assert!(cb.allow(), "冷却到期应进入 HalfOpen 并放行单探测");
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn half_open_success_closes() {
        let cb = breaker_with(20);
        for _ in 0..3 {
            cb.on_failure();
        }
        std::thread::sleep(Duration::from_millis(30));
        assert!(cb.allow()); // → HalfOpen
        cb.on_success();
        assert_eq!(cb.state(), CircuitState::Closed, "探测成功应恢复 Closed");
        assert!(cb.allow());
    }

    #[test]
    fn half_open_failure_reopens_immediately() {
        let cb = breaker_with(20);
        for _ in 0..3 {
            cb.on_failure();
        }
        std::thread::sleep(Duration::from_millis(30));
        assert!(cb.allow()); // → HalfOpen
        cb.on_failure(); // HalfOpen 下失败 → 立即重新 Open（不看计数）
        assert!(!cb.allow(), "HalfOpen 探测失败应立即回到 Open");
        assert!(matches!(cb.state(), CircuitState::Open { .. }));
    }

    #[test]
    fn intermittent_failures_reset_by_success() {
        let cb = CircuitBreaker::new();
        cb.on_failure();
        cb.on_failure();
        cb.on_success(); // 计数清零
        cb.on_failure();
        cb.on_failure();
        assert!(cb.allow(), "成功后重新计数，2 次失败不应熔断");
        cb.on_failure();
        assert!(!cb.allow());
    }
}
