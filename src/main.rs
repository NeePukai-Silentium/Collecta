#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Collecta layout mockup: all pages, menus and animations on made-up data.
//! `collecta.exe [--theme <theme.ini>] [--log <file>]`

mod app;
mod gallery;
mod mock;
mod theme;
mod thumbs;
mod timing;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use slint::winit_030::{EventResult, WinitWindowAccessor, winit};
use slint::{ComponentHandle, LogicalSize, Timer};

slint::include_modules!();

const WINDOW_SIZE: LogicalSize = LogicalSize::new(1280.0, 800.0);

thread_local! {
    static APP: RefCell<Option<Rc<app::App>>> = const { RefCell::new(None) };
}

/// For results coming back from background threads.
pub fn with_app(action: impl FnOnce(&app::App)) {
    let app = APP.with(|app| app.borrow().clone());
    if let Some(app) = app {
        action(&app);
    }
}

fn main() {
    let mut theme_path = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--theme" {
            theme_path = args.next().map(PathBuf::from);
        } else if arg == "--log" {
            if let Some(log) = args.next() {
                timing::set_log_path(PathBuf::from(log));
            }
        }
    }

    let _ = slint::BackendSelector::new().backend_name("winit".into()).renderer_name("software".into()).select();
    let window = MainWindow::new().expect("failed to create window");
    window.window().set_size(WINDOW_SIZE);

    let theme_file = theme_path.map(|path| theme::ThemeFile::new(&window, path));
    let pictures = theme_file.as_ref().and_then(|file| file.images_dir()).map(|dir| thumbs::list(&dir)).unwrap_or_default();
    let _theme_watch = theme_file.map(|file| theme::watch(&window, file));

    let app = app::App::new(&window, pictures);
    APP.with(|slot| *slot.borrow_mut() = Some(app.clone()));
    thumbs::spawn_loading(app.picture_paths().to_vec(), |index, width, height, thumbnail| {
        let _ = slint::invoke_from_event_loop(move || {
            with_app(|app| app.picture_loaded(index, width, height, thumbnail));
        });
    });

    log_first_frame(&window);
    window.run().expect("event loop failed");
    APP.with(|slot| slot.borrow_mut().take());
}

/// The first frame is drawn while handling the first redraw request; the timer fires right after it.
fn log_first_frame(window: &MainWindow) {
    let mut logged = false;
    window.window().on_winit_window_event(move |_, event| {
        if !logged && matches!(event, winit::event::WindowEvent::RedrawRequested) {
            logged = true;
            Timer::single_shot(Duration::ZERO, || {
                timing::write_log(&format!(
                    "first frame {} ms after process start, {}",
                    timing::since_process_start(),
                    timing::memory_usage()
                ));
                Timer::single_shot(Duration::from_secs(2), || {
                    timing::write_log(&format!("2 s after the first frame: {}", timing::memory_usage()));
                });
            });
        }
        EventResult::Propagate
    });
}
