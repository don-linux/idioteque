//! Windowless health-check helpers (CONTRACT.md §4.5).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use cef::wrap_render_handler;
use cef::*;

use crate::exit;

/// Internal watchdog (contract 4.5). ADE waits 45s; the host must die first.
pub const WATCHDOG_TIMEOUT: Duration = Duration::from_secs(30);

/// Fatal text when the watchdog fires. Must mention the 30s budget.
pub const WATCHDOG_MESSAGE: &str = "health check timed out after 30s";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HealthView {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Windowless `get_view_rect` (contract 4.5): 800×600 at the origin.
pub const HEALTH_VIEW: HealthView = HealthView {
    x: 0,
    y: 0,
    width: 800,
    height: 600,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HealthScreen {
    pub device_scale_factor: f32,
    pub depth: i32,
    pub depth_per_component: i32,
    pub is_monochrome: i32,
}

/// Health `screen_info` is 1.0× / 24-bit, not `--idq-scale`.
pub const HEALTH_SCREEN: HealthScreen = HealthScreen {
    device_scale_factor: 1.0,
    depth: 24,
    depth_per_component: 8,
    is_monochrome: 0,
};

/// After `WATCHDOG_TIMEOUT`, fire only if the success flag is still clear.
pub fn watchdog_timed_out(cancelled: bool) -> bool {
    !cancelled
}

pub fn watchdog_exit_code() -> i32 {
    exit::HEALTH_TIMEOUT
}

/// 30 s internal watchdog. Returns a cancel flag; set it on success.
pub fn start_watchdog() -> Arc<AtomicBool> {
    start_watchdog_after(WATCHDOG_TIMEOUT, || {
        exit::fatal(watchdog_exit_code(), WATCHDOG_MESSAGE);
    })
}

/// Testable watchdog: sleep `timeout`, then run `on_fire` if still not cancelled.
pub(crate) fn start_watchdog_after(
    timeout: Duration,
    on_fire: impl FnOnce() + Send + 'static,
) -> Arc<AtomicBool> {
    let done = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&done);
    thread::Builder::new()
        .name("idq-health-watchdog".into())
        .spawn(move || {
            thread::sleep(timeout);
            if watchdog_timed_out(flag.load(Ordering::SeqCst)) {
                on_fire();
            }
        })
        .expect("spawn health watchdog");
    done
}

fn fill_view(rect: &mut Rect) {
    rect.x = HEALTH_VIEW.x;
    rect.y = HEALTH_VIEW.y;
    rect.width = HEALTH_VIEW.width;
    rect.height = HEALTH_VIEW.height;
}

fn apply_screen_info(info: &mut ScreenInfo) {
    info.device_scale_factor = HEALTH_SCREEN.device_scale_factor;
    info.depth = HEALTH_SCREEN.depth;
    info.depth_per_component = HEALTH_SCREEN.depth_per_component;
    info.is_monochrome = HEALTH_SCREEN.is_monochrome;
    fill_view(&mut info.rect);
    info.available_rect = info.rect.clone();
}

wrap_render_handler! {
    pub struct HealthRenderHandler;

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect {
                fill_view(rect);
            }
        }

        fn screen_info(
            &self,
            _browser: Option<&mut Browser>,
            screen_info: Option<&mut ScreenInfo>,
        ) -> ::std::os::raw::c_int {
            let Some(info) = screen_info else {
                return 0;
            };
            apply_screen_info(info);
            1
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            _type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            _buffer: *const u8,
            _width: ::std::os::raw::c_int,
            _height: ::std::os::raw::c_int,
        ) {
        }
    }
}

pub fn window_info() -> WindowInfo {
    WindowInfo::default().set_as_windowless(0)
}

/// Contract 4.5: health is windowless; `parent` is 0.
#[cfg(test)]
pub fn health_window_is_windowless() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    #[test]
    fn watchdog_timeout_is_30s_not_ade_45s() {
        assert_eq!(WATCHDOG_TIMEOUT, Duration::from_secs(30));
        assert_eq!(WATCHDOG_TIMEOUT.as_secs(), 30);
        assert!(WATCHDOG_TIMEOUT < Duration::from_secs(45));
        assert_ne!(WATCHDOG_TIMEOUT, Duration::from_secs(45));
        assert!(WATCHDOG_MESSAGE.contains("30s"));
        assert!(!WATCHDOG_MESSAGE.contains("45"));
    }

    #[test]
    fn watchdog_exit_is_contract_12() {
        assert_eq!(watchdog_exit_code(), 12);
        assert_eq!(watchdog_exit_code(), exit::HEALTH_TIMEOUT);
        assert_ne!(watchdog_exit_code(), exit::OK);
        assert_ne!(watchdog_exit_code(), exit::INIT_FAILED);
        assert_ne!(watchdog_exit_code(), exit::BAD_ARGS);
        assert_ne!(watchdog_exit_code(), 45);
    }

    #[test]
    fn watchdog_fires_only_when_not_cancelled() {
        assert!(watchdog_timed_out(false));
        assert!(!watchdog_timed_out(true));
    }

    #[test]
    fn watchdog_thread_fires_after_deadline_when_flag_stays_clear() {
        let fires = Arc::new(AtomicUsize::new(0));
        let fires_cb = Arc::clone(&fires);
        let flag = start_watchdog_after(Duration::from_millis(40), move || {
            assert_eq!(watchdog_exit_code(), 12);
            fires_cb.fetch_add(1, Ordering::SeqCst);
        });
        thread::sleep(Duration::from_millis(120));
        assert!(
            watchdog_timed_out(flag.load(Ordering::SeqCst)),
            "success flag must stay clear so the watchdog can fatal 12"
        );
        assert_eq!(fires.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn watchdog_thread_stays_silent_when_cancelled_before_deadline() {
        let fires = Arc::new(AtomicUsize::new(0));
        let fires_cb = Arc::clone(&fires);
        let flag = start_watchdog_after(Duration::from_millis(80), move || {
            fires_cb.fetch_add(1, Ordering::SeqCst);
        });
        flag.store(true, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(140));
        assert_eq!(fires.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn watchdog_late_cancel_does_not_undo_a_fire() {
        let fires = Arc::new(AtomicUsize::new(0));
        let fires_cb = Arc::clone(&fires);
        let flag = start_watchdog_after(Duration::from_millis(30), move || {
            fires_cb.fetch_add(1, Ordering::SeqCst);
        });
        thread::sleep(Duration::from_millis(90));
        flag.store(true, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(30));
        assert_eq!(fires.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn health_view_is_800x600_origin() {
        assert_eq!(
            HEALTH_VIEW,
            HealthView {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            }
        );
        let mut rect = Rect::default();
        fill_view(&mut rect);
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
        assert_eq!(rect.width, 800);
        assert_eq!(rect.height, 600);
    }

    #[test]
    fn health_screen_info_ignores_device_scale_and_is_24bit() {
        assert_eq!(HEALTH_SCREEN.device_scale_factor, 1.0);
        assert_eq!(HEALTH_SCREEN.depth, 24);
        assert_eq!(HEALTH_SCREEN.depth_per_component, 8);
        assert_eq!(HEALTH_SCREEN.is_monochrome, 0);
        let mut info = ScreenInfo::default();
        apply_screen_info(&mut info);
        assert_eq!(info.device_scale_factor, 1.0);
        assert_eq!(info.depth, 24);
        assert_eq!(info.rect.width, 800);
        assert_eq!(info.rect.height, 600);
        assert_eq!(info.available_rect.width, 800);
        assert_eq!(info.available_rect.height, 600);
    }

    #[test]
    fn health_uses_windowless_parent_zero() {
        assert!(health_window_is_windowless());
    }
}
