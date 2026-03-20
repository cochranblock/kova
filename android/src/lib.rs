//! Kova Android entry point. egui GUI on Pixel 9 XL Pro via NativeActivity.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use android_activity::AndroidApp;

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    // Init Android logging
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    log::info!("kova android starting");

    // Set HOME to Android internal storage so kova paths resolve correctly
    if let Some(path) = app.internal_data_path() {
        unsafe { std::env::set_var("HOME", &path) };
        log::info!("HOME={}", path.display());
    }

    // Bootstrap kova dirs
    if let Err(e) = kova::bootstrap() {
        log::error!("bootstrap failed: {}", e);
    }

    // Keep a clone for soft keyboard control
    let kb_app = app.clone();

    let options = eframe::NativeOptions {
        android_app: Some(app),
        ..Default::default()
    };

    eframe::run_native(
        "Kova",
        options,
        Box::new(move |cc| {
            // Scale for high-DPI mobile screens
            // Pixel 9 XL Pro: 486 PPI. Default egui is ~1.0 which is tiny.
            // 2.5 gives readable UI on flagship phones.
            cc.egui_ctx.set_pixels_per_point(2.5);

            kova::theme::f320(&cc.egui_ctx);

            let app = kova::gui::KovaApp::new(cc, false);
            Ok(Box::new(MobileApp {
                inner: app,
                android_app: kb_app,
                keyboard_visible: false,
            }))
        }),
    )
    .expect("eframe failed");
}

/// Wrapper that handles Android-specific concerns (soft keyboard, safe areas).
struct MobileApp {
    inner: kova::gui::KovaApp,
    android_app: AndroidApp,
    keyboard_visible: bool,
}

impl eframe::App for MobileApp {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        // Soft keyboard: show when egui wants text input, hide otherwise
        let wants_kb = ctx.wants_keyboard_input();
        if wants_kb && !self.keyboard_visible {
            self.android_app.show_soft_input(true);
            self.keyboard_visible = true;
        } else if !wants_kb && self.keyboard_visible {
            self.android_app.hide_soft_input(false);
            self.keyboard_visible = false;
        }

        // Delegate to inner KovaApp
        self.inner.update(ctx, frame);
    }
}
