//! Kova web client. Pure Rust (egui) compiled to WASM. Connects to kova serve API.

#![allow(non_camel_case_types)]

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

mod app;
mod theme;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// t134=WebHandle
/// Web handle for JavaScript to start the app.
#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
#[wasm_bindgen]
pub struct t134 {
    runner: eframe::WebRunner,
}

#[cfg(target_arch = "wasm32")]
impl Default for t134 {
    fn default() -> Self { Self::new() }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl t134 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        eframe::WebLogger::init(log::LevelFilter::Info).ok();
        Self {
            runner: eframe::WebRunner::new(),
        }
    }

    #[wasm_bindgen]
    pub async fn start(&self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
        self.runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    theme::f221(&cc.egui_ctx);
                    Ok(Box::new(app::t135::new(cc)))
                }),
            )
            .await
    }

    #[wasm_bindgen]
    pub fn destroy(&self) {
        self.runner.destroy();
    }
}