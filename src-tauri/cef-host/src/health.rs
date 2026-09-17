//! Windowless health-check helpers (CONTRACT.md §4.5).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use cef::wrap_render_handler;
use cef::*;

use crate::exit;

/// 30 s internal watchdog. Returns a cancel flag; set it on success.
pub fn start_watchdog() -> Arc<AtomicBool> {
    let done = Arc::new(AtomicBool::new(false));
    let flag = done.clone();
    thread::Builder::new()
        .name("idq-health-watchdog".into())
        .spawn(move || {
            thread::sleep(Duration::from_secs(30));
            if !flag.load(Ordering::SeqCst) {
                exit::fatal(
                    exit::HEALTH_TIMEOUT,
                    "health check timed out after 30s",
                );
            }
        })
        .expect("spawn health watchdog");
    done
}

fn fill_view(rect: &mut Rect) {
    rect.x = 0;
    rect.y = 0;
    rect.width = 800;
    rect.height = 600;
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
            info.device_scale_factor = 1.0;
            info.depth = 24;
            info.depth_per_component = 8;
            info.is_monochrome = 0;
            fill_view(&mut info.rect);
            info.available_rect = info.rect.clone();
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
