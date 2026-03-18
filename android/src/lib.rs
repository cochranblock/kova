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
        // SAFETY: called at startup before any threads spawn, single-threaded at this point.
        unsafe { std::env::set_var("HOME", &path) };
        log::info!("HOME={}", path.display());
    }

    // Bootstrap kova dirs (creates ~/.kova/, prompts, etc.)
    if let Err(e) = kova::bootstrap() {
        log::error!("bootstrap failed: {}", e);
    }

    // Run egui GUI
    let options = eframe::NativeOptions {
        android_app: Some(app),
        ..Default::default()
    };

    eframe::run_native(
        "Kova",
        options,
        Box::new(move |cc| {
            kova::theme::f320(&cc.egui_ctx);
            Ok(Box::new(kova::gui::KovaApp::new(cc, false)))
        }),
    )
    .expect("eframe failed");
}