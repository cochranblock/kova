/* tslint:disable */
/* eslint-disable */

/**
 * t134=WebHandle
 * Web handle for JavaScript to start the app.
 */
export class t134 {
    free(): void;
    [Symbol.dispose](): void;
    destroy(): void;
    constructor();
    start(canvas: HTMLCanvasElement): Promise<void>;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_t134_free: (a: number, b: number) => void;
    readonly t134_destroy: (a: number) => void;
    readonly t134_new: () => number;
    readonly t134_start: (a: number, b: any) => any;
    readonly wasm_bindgen__closure__destroy__h1b10d9879960e4c5: (a: number, b: number) => void;
    readonly wasm_bindgen__closure__destroy__h398b7fb26c99c3ae: (a: number, b: number) => void;
    readonly wasm_bindgen__closure__destroy__h178aa722dd08b595: (a: number, b: number) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h67500899e85f003c: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen__convert__closures_____invoke__h671261e8779f5c0b: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h2d41965e7eedbd9d: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h2d41965e7eedbd9d_1: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__hff86d667ec426926: (a: number, b: number) => [number, number];
    readonly wasm_bindgen__convert__closures_____invoke__hc16cc9dbbaf5e057: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__hc16cc9dbbaf5e057_5: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__hc16cc9dbbaf5e057_6: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h4fb7439fbc64fa9a: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
