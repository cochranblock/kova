// Unlicense — cochranblock.org
//! Kova web client. Pure Rust (egui) compiled to WASM. Connects to kova serve API.

mod app;
mod theme;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Web handle for JavaScript to start the app.
#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WebHandle {
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
                    theme::apply(&cc.egui_ctx);
                    Ok(Box::new(app::KovaWebApp::new(cc)))
                }),
            )
            .await
    }

    #[wasm_bindgen]
    pub fn destroy(&self) {
        self.runner.destroy();
    }
}
