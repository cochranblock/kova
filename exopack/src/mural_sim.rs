// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Mural UI Quality Simulation — source-level analysis of mural-wasm rendering quality.
//!
//! Checks sprite rendering, animation timing, movement physics, draw pipeline,
//! atlas layout, scene triggers, JS bridge, memory safety, pet state machine,
//! and WASM build target.
//!
//! Standalone sim: run against oakilydokily repo root.
//! Integrated into TRIPLE SIMS as Sim 4.

use std::path::Path;

use crate::triple_sims::{Finding, Severity, SimResult};

// ── Helpers ──────────────────────────────────────────────────────────────

fn read_mural_src(root: &Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join("mural-wasm").join(rel)).ok()
}

fn mural_contains(root: &Path, rel: &str, pattern: &str) -> bool {
    read_mural_src(root, rel).is_some_and(|s| s.contains(pattern))
}

fn mural_contains_any(root: &Path, rel: &str, patterns: &[&str]) -> bool {
    read_mural_src(root, rel).is_some_and(|s| patterns.iter().any(|p| s.contains(p)))
}

fn read_asset(root: &Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join("assets").join(rel)).ok()
}

fn asset_contains(root: &Path, rel: &str, pattern: &str) -> bool {
    read_asset(root, rel).is_some_and(|s| s.contains(pattern))
}

fn finding(ok: bool, area: &str, pass_msg: &str, fail_msg: &str) -> Finding {
    Finding {
        sim: 4,
        severity: if ok { Severity::Pass } else { Severity::Fail },
        area: area.to_string(),
        message: (if ok { pass_msg } else { fail_msg }).to_string(),
    }
}

fn warn(area: &str, msg: &str) -> Finding {
    Finding { sim: 4, severity: Severity::Warning, area: area.to_string(), message: msg.to_string() }
}

// ── Mural UI Quality Sim (f173) ──────────────────────────────────────────

/// f173=sim4_mural_ui_quality. Source-level rendering quality analysis.
pub fn f173_sim4_mural_ui_quality(oakily_root: &Path) -> SimResult {
    let mut findings = Vec::new();

    let mural_dir = oakily_root.join("mural-wasm");
    if !mural_dir.exists() {
        findings.push(Finding {
            sim: 4, severity: Severity::Fail,
            area: "4-prereq".to_string(),
            message: format!("mural-wasm directory not found at {}", mural_dir.display()),
        });
        return SimResult { sim: 4, name: "Mural UI Quality".to_string(), findings };
    }

    // ── 4A: Sprite Rendering Quality ──────────────────────────────────
    // Crisp pixel art requires FilterMode::Nearest (no bilinear blur)
    findings.push(finding(
        mural_contains(oakily_root, "src/sprites.rs", "FilterMode::Nearest"),
        "4A-sprite-filter",
        "Sprites use FilterMode::Nearest — crisp pixel art",
        "Sprites missing FilterMode::Nearest — bilinear blur will smear pixels",
    ));

    findings.push(finding(
        mural_contains(oakily_root, "src/landscape.rs", "FilterMode::Nearest"),
        "4A-landscape-filter",
        "Landscape uses FilterMode::Nearest — crisp background",
        "Landscape missing FilterMode::Nearest — blurry background",
    ));

    // Sprite dest size 32x48 (proper 2x scale for 16x24 pixel art)
    findings.push(finding(
        mural_contains(oakily_root, "src/pet.rs", "vec2(32., 48.)"),
        "4A-sprite-dest-size",
        "Sprite dest_size 32x48 — correct 2x scale",
        "Sprite dest_size not 32x48 — scaling mismatch",
    ));

    // Sprite centering: draw at pos - (16, 24) = half dest_size
    findings.push(finding(
        mural_contains_any(oakily_root, "src/pet.rs", &["self.pos.x - 16.", "self.pos.y - 24."]),
        "4A-sprite-center",
        "Sprites centered at pos - half-size",
        "Sprite draw offset incorrect — pets will appear misaligned",
    ));

    // ── 4B: Animation Frame Timing ───────────────────────────────────
    // 0.15s per frame = 6.67fps animation (standard for pixel art walk cycles)
    findings.push(finding(
        mural_contains(oakily_root, "src/pet.rs", "0.15"),
        "4B-anim-timing",
        "Animation timer 0.15s/frame — 6.67fps pixel art standard",
        "Animation timer not 0.15s — non-standard frame rate",
    ));

    // 4-frame cycle (% 4)
    findings.push(finding(
        mural_contains(oakily_root, "src/pet.rs", "% 4"),
        "4B-anim-cycle",
        "4-frame animation cycle",
        "Animation cycle not 4 frames — may stutter or skip",
    ));

    // dt-based timing (frame-rate independent)
    findings.push(finding(
        mural_contains(oakily_root, "src/pet.rs", "self.anim_timer += dt"),
        "4B-dt-timing",
        "Animation timer is dt-based — frame-rate independent",
        "Animation not dt-based — will run at different speeds on different hardware",
    ));

    // ── 4C: Movement Physics ─────────────────────────────────────────
    // Wandering: velocity * dt (frame-rate independent)
    findings.push(finding(
        mural_contains(oakily_root, "src/pet.rs", "self.vel * dt"),
        "4C-movement-dt",
        "Movement uses vel * dt — frame-rate independent",
        "Movement not velocity*dt — will jitter at varying framerates",
    ));

    // Exodus velocity: -80 px/sec (smooth leftward exit)
    findings.push(finding(
        mural_contains(oakily_root, "src/pet.rs", "-80."),
        "4C-exodus-vel",
        "Exodus velocity -80 px/sec — smooth exit speed",
        "Exodus velocity missing or wrong — pets may teleport or crawl off-screen",
    ));

    // Bounce at screen edges (prevents pets from leaving viewport during wander)
    findings.push(finding(
        mural_contains_any(oakily_root, "src/pet.rs", &["-self.vel.x", "self.vel.x = -self.vel.x"]),
        "4C-bounce",
        "Pets bounce at screen edges during wander",
        "No edge bounce — pets will walk off-screen during wander",
    ));

    // Edge padding (32px matches half sprite width)
    findings.push(finding(
        mural_contains_any(oakily_root, "src/pet.rs", &["self.pos.x < 32.", "w - 32."]),
        "4C-edge-padding",
        "Edge padding 32px — matches sprite half-width",
        "Edge padding incorrect — pets will clip screen edges",
    ));

    // Heart particle physics: gravity + lifetime
    findings.push(finding(
        mural_contains(oakily_root, "src/pet.rs", "h.vel.y -= 50."),
        "4C-heart-gravity",
        "Heart particles have gravity (50 px/s^2) — natural arc",
        "Hearts missing gravity — will float away linearly",
    ));

    findings.push(finding(
        mural_contains(oakily_root, "src/pet.rs", "h.life -= dt"),
        "4C-heart-lifetime",
        "Hearts decay over time — finite lifetime",
        "Hearts missing lifetime decay — will accumulate forever",
    ));

    // ── 4D: Draw Pipeline ────────────────────────────────────────────
    // Transparent clear when landscape loaded
    findings.push(finding(
        mural_contains(oakily_root, "src/main.rs", "Color::from_rgba(0, 0, 0, 0)"),
        "4D-transparent-clear",
        "Clears with transparent alpha — mural.png shows through CSS",
        "Missing transparent clear — canvas will be opaque",
    ));

    // Fallback solid color
    findings.push(finding(
        mural_contains(oakily_root, "src/main.rs", "0xe8, 0xee, 0xf2"),
        "4D-fallback-color",
        "Fallback gray (#e8eef2) when landscape fails",
        "No fallback color — canvas will be black on load failure",
    ));

    // Draw order: landscape → scene → pets (correct z-order)
    if let Some(main_src) = read_mural_src(oakily_root, "src/main.rs") {
        let landscape_draw = main_src.find("draw_texture_ex");
        let scene_draw = main_src.find("scene.draw()");
        let pet_draw = main_src.rfind("pets[i].draw");
        let correct_order = match (landscape_draw, scene_draw, pet_draw) {
            (Some(a), Some(b), Some(c)) => a < b && b < c,
            _ => false,
        };
        findings.push(finding(
            correct_order,
            "4D-draw-order",
            "Draw order: landscape → scene → pets (correct z-order)",
            "Draw order incorrect — z-fighting or visual artifacts",
        ));
    }

    // Occlusion culling: viewport check
    findings.push(finding(
        mural_contains(oakily_root, "src/main.rs", "viewport.contains(p.pos)"),
        "4D-occlusion-cull",
        "Occlusion culling: only draw pets in viewport",
        "No occlusion culling — drawing off-screen pets wastes GPU",
    ));

    // Exodus off-screen skip
    findings.push(finding(
        mural_contains(oakily_root, "src/pet.rs", "self.pos.x < -50."),
        "4D-exodus-skip",
        "Exodus pets stop drawing at x < -50 — clean exit",
        "Exodus pets drawn forever — wasted draw calls",
    ));

    // framebuffer_alpha: true in window config
    findings.push(finding(
        mural_contains(oakily_root, "src/main.rs", "framebuffer_alpha: true"),
        "4D-framebuffer-alpha",
        "Framebuffer alpha enabled — transparency works",
        "Framebuffer alpha not set — canvas will be opaque despite clear(0,0,0,0)",
    ));

    // ── 4E: Atlas Layout ─────────────────────────────────────────────
    // Grid: 4 cols, 3 rows
    let sprites = read_mural_src(oakily_root, "src/sprites.rs").unwrap_or_default();
    let has_4_cols = sprites.contains("cols: 4") || sprites.contains("cols = 4");
    let has_3_rows = sprites.contains("rows: 3") || sprites.contains("rows = 3");
    findings.push(finding(
        has_4_cols && has_3_rows,
        "4E-atlas-grid",
        "Atlas grid 4x3 (12 cells) — fits species×anim layout",
        "Atlas grid not 4x3 — sprite mapping will be wrong",
    ));

    // Species→row mapping
    findings.push(finding(
        sprites.contains("Cat => 0") && sprites.contains("Dog => 1") && sprites.contains("GuineaPig => 2"),
        "4E-species-rows",
        "Species→row: Cat=0, Dog=1, GuineaPig=2",
        "Species row mapping incorrect — wrong sprites for wrong animals",
    ));

    // Animation→col mapping
    findings.push(finding(
        sprites.contains("Walk => 0") && sprites.contains("Interaction => 1") && sprites.contains("Sleeping => 2"),
        "4E-anim-cols",
        "Animation→col: Walk=0, Interaction=1, Sleeping=2",
        "Animation column mapping incorrect — wrong anims for states",
    ));

    // Frame wrapping (% cols prevents OOB texture read)
    findings.push(finding(
        sprites.contains("% self.sheet.cols"),
        "4E-frame-wrap",
        "Frame wraps at col count — prevents OOB texture read",
        "No frame wrapping — will read garbage pixels past atlas edge",
    ));

    // Kiss frame bounds: .min(frame) caps at col 3
    findings.push(finding(
        sprites.contains("3.min(frame)"),
        "4E-kiss-bounds",
        "Kiss frame bounded to col 3 — no OOB",
        "Kiss frame unbounded — may read past atlas",
    ));

    // ── 4F: Scene Triggers ───────────────────────────────────────────
    let scenes = read_mural_src(oakily_root, "src/scenes.rs").unwrap_or_default();

    findings.push(finding(
        scenes.contains("scroll_x > 100."),
        "4F-cozy-nook-threshold",
        "Cozy Nook triggers at scroll_x > 100",
        "Cozy Nook threshold missing — scene won't trigger",
    ));

    findings.push(finding(
        scenes.contains("scroll_y > 300."),
        "4F-tubing-threshold",
        "Winter Tubing triggers at scroll_y > 300",
        "Tubing threshold missing",
    ));

    findings.push(finding(
        scenes.contains("scroll_y > 800."),
        "4F-doggy-door-threshold",
        "Doggy Door triggers at scroll_y > 800 (footer)",
        "Doggy Door threshold missing — exodus won't fire",
    ));

    // Cozy nook easing (lerp factor 0.1 = smooth slide)
    findings.push(finding(
        scenes.contains("* 0.1"),
        "4F-cozy-nook-easing",
        "Cozy Nook uses 0.1 lerp — smooth slide-in",
        "Cozy Nook has no easing — abrupt pop-in",
    ));

    // Tubing momentum (velocity accumulates)
    findings.push(finding(
        scenes.contains("tubing_vel += 2.") || scenes.contains("tubing_vel +="),
        "4F-tubing-momentum",
        "Tubing has momentum accumulation",
        "Tubing has no momentum — unrealistic physics",
    ));

    // Doggy door one-shot check (main.rs: was_triggered guard)
    findings.push(finding(
        mural_contains(oakily_root, "src/main.rs", "was_triggered"),
        "4F-doggy-door-oneshot",
        "Doggy door exodus is one-shot (was_triggered guard)",
        "Doggy door may re-trigger — pets will glitch on scroll bounce",
    ));

    // ── 4G: JS Bridge Integrity ──────────────────────────────────────
    let bridge = read_mural_src(oakily_root, "src/bridge.rs").unwrap_or_default();

    // thread_local Cell (WASM-safe, no mutex overhead)
    findings.push(finding(
        bridge.contains("thread_local!") && bridge.contains("Cell<f32>"),
        "4G-thread-local-cells",
        "Bridge uses thread_local Cells — WASM-safe state",
        "Bridge not using thread_local Cells — unsafe in WASM",
    ));

    // #[no_mangle] extern "C" (miniquad compat, no wasm-bindgen)
    findings.push(finding(
        bridge.contains("#[no_mangle]") && bridge.contains("extern \"C\""),
        "4G-no-mangle-extern",
        "FFI: #[no_mangle] extern \"C\" — miniquad gl.js compatible",
        "FFI missing no_mangle or extern C — JS won't find exports",
    ));

    // cfg(target_arch = "wasm32") guards
    findings.push(finding(
        bridge.contains("cfg(target_arch = \"wasm32\")"),
        "4G-wasm32-guard",
        "FFI exports guarded by cfg(wasm32) — clean native build",
        "FFI exports not cfg-guarded — will fail on native",
    ));

    // JS: 50ms poll interval
    findings.push(finding(
        asset_contains(oakily_root, "mural-bridge.js", "50"),
        "4G-js-poll-interval",
        "JS polls scroll at 50ms (20Hz) — responsive without thrashing",
        "JS poll interval not 50ms",
    ));

    // JS: try/catch error handling
    findings.push(finding(
        asset_contains(oakily_root, "mural-bridge.js", "try {"),
        "4G-js-error-handling",
        "JS bridge has try/catch — won't crash on WASM failure",
        "JS bridge missing error handling — WASM load failure will crash page",
    ));

    // JS: mouse scale mapping
    findings.push(finding(
        asset_contains(oakily_root, "mural-bridge.js", "cw / w"),
        "4G-js-mouse-scale",
        "Mouse coords scale-mapped (canvas/display ratio) — HiDPI correct",
        "Mouse coords not scale-mapped — will be wrong on retina/HiDPI",
    ));

    // ── 4H: Memory Safety ────────────────────────────────────────────
    // Hearts cleanup: retain_mut with lifetime check
    findings.push(finding(
        mural_contains(oakily_root, "src/pet.rs", "retain_mut"),
        "4H-hearts-cleanup",
        "Hearts use retain_mut — dead particles removed each frame",
        "Hearts not cleaned up — unbounded memory growth",
    ));

    findings.push(finding(
        mural_contains(oakily_root, "src/pet.rs", "h.life > 0."),
        "4H-hearts-lifetime-check",
        "Hearts filtered by life > 0 — finite lifespan",
        "Hearts missing lifetime check in retain",
    ));

    // Landscape is Option (graceful load failure)
    findings.push(finding(
        mural_contains(oakily_root, "src/landscape.rs", "Option<Texture2D>"),
        "4H-landscape-option",
        "Landscape is Option — graceful fallback on load failure",
        "Landscape not optional — will panic on missing mural.png",
    ));

    // ── 4I: Pet State Machine ────────────────────────────────────────
    let pet = read_mural_src(oakily_root, "src/pet.rs").unwrap_or_default();

    // All 4 states handled in update match
    let all_states = pet.contains("PetState::Wandering")
        && pet.contains("PetState::Sleeping")
        && pet.contains("PetState::Interacting")
        && pet.contains("PetState::Exodus");
    findings.push(finding(
        all_states,
        "4I-state-coverage",
        "All 4 pet states handled (Wandering, Sleeping, Interacting, Exodus)",
        "Missing pet state coverage — unhandled state will cause default behavior",
    ));

    // Proximity detection: same species + distance
    findings.push(finding(
        mural_contains(oakily_root, "src/main.rs", "pi == pj") && mural_contains(oakily_root, "src/main.rs", "distance"),
        "4I-proximity",
        "Proximity detection: same species + distance check",
        "Proximity detection missing — pets won't interact",
    ));

    // Proximity threshold: 30px
    findings.push(finding(
        mural_contains(oakily_root, "src/main.rs", "< 30."),
        "4I-proximity-threshold",
        "Proximity threshold 30px — appropriate for 32px sprites",
        "Proximity threshold missing or wrong",
    ));

    // Guinea pig kiss special case
    findings.push(finding(
        mural_contains(oakily_root, "src/main.rs", "Species::GuineaPig") && mural_contains(oakily_root, "src/main.rs", "trigger_kiss"),
        "4I-guinea-pig-kiss",
        "Guinea pig kiss interaction with hearts",
        "Guinea pig kiss missing — lost character charm",
    ));

    // ── 4J: WASM Build Target ────────────────────────────────────────
    let cargo = std::fs::read_to_string(oakily_root.join("mural-wasm/Cargo.toml")).unwrap_or_default();

    // macroquad dependency
    findings.push(finding(
        cargo.contains("macroquad"),
        "4J-macroquad-dep",
        "macroquad dependency present",
        "macroquad missing — mural won't render",
    ));

    // No wasm-bindgen (miniquad native approach)
    findings.push(finding(
        !cargo.contains("wasm-bindgen"),
        "4J-no-wasm-bindgen",
        "No wasm-bindgen — uses miniquad gl.js (lighter, no npm)",
        "wasm-bindgen present — contradicts miniquad architecture",
    ));

    // Binary target exists
    findings.push(finding(
        cargo.contains("[[bin]]") && cargo.contains("mural-wasm"),
        "4J-bin-target",
        "Binary target mural-wasm defined",
        "Binary target missing — cargo build won't produce WASM",
    ));

    // Test harness exists
    findings.push(finding(
        oakily_root.join("assets/mural-test.html").exists(),
        "4J-test-harness",
        "mural-test.html test harness present",
        "mural-test.html missing — no way to test in browser",
    ));

    // WASM binary exists (built artifact)
    let wasm_exists = oakily_root.join("assets/mural-wasm.wasm").exists();
    if wasm_exists {
        findings.push(Finding {
            sim: 4, severity: Severity::Pass,
            area: "4J-wasm-built".to_string(),
            message: "mural-wasm.wasm compiled artifact present".to_string(),
        });
    } else {
        findings.push(warn("4J-wasm-built", "mural-wasm.wasm not found — needs dm (build alias)"));
    }

    SimResult { sim: 4, name: "Mural UI Quality".to_string(), findings }
}

/// Discover oakilydokily directory as sibling of project root.
pub fn find_oakily_root(kova_root: &Path) -> Option<std::path::PathBuf> {
    let parent = kova_root.parent()?;
    let oakily = parent.join("oakilydokily");
    if oakily.exists() { Some(oakily) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mural_sim_runs_without_panic() {
        let kova = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
        if let Some(oakily) = find_oakily_root(&kova) {
            let result = f173_sim4_mural_ui_quality(&oakily);
            assert_eq!(result.sim, 4);
            assert!(!result.findings.is_empty());
            let pass = result.findings.iter().filter(|f| f.severity == Severity::Pass).count();
            assert!(pass > 20, "Expected at least 20 passes, got {}", pass);
        } else {
            eprintln!("oakilydokily not found — skipping mural sim test");
        }
    }
}
