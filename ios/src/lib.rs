//! Kova iOS entry point. egui GUI via eframe on iOS.
//!
//! Build: cargo build --target aarch64-apple-ios --release
//! Links as a static library into the Xcode project.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::ffi::c_void;

/// Called from Swift/ObjC to launch the egui app.
/// The Xcode project passes the UIWindow pointer.
#[unsafe(no_mangle)]
pub extern "C" fn kova_ios_main() {
    // Init iOS logging
    oslog::OsLogger::new("org.cochranblock.kova")
        .level_filter(log::LevelFilter::Info)
        .init()
        .ok();

    log::info!("kova iOS starting");

    // Set HOME to app sandbox Documents dir
    if let Some(home) = dirs::home_dir() {
        unsafe { std::env::set_var("HOME", &home) };
        log::info!("HOME={}", home.display());
    }

    // Bootstrap kova dirs
    if let Err(e) = kova::bootstrap() {
        log::error!("bootstrap failed: {}", e);
    }

    let options = eframe::NativeOptions {
        // CRITICAL: run_and_return must be false on iOS.
        // If true, eframe returns after first frame, the caller re-invokes
        // kova_ios_main, and the app enters infinite recursion → stack overflow on M1.
        run_and_return: false,
        ..Default::default()
    };

    eframe::run_native(
        "Kova",
        options,
        Box::new(move |cc| {
            // iOS scale: use system scale factor
            cc.egui_ctx.set_pixels_per_point(2.0);
            kova::theme::f320(&cc.egui_ctx);
            let app = kova::gui::KovaApp::new(cc, false);
            Ok(Box::new(app))
        }),
    )
    .expect("eframe failed");
}
