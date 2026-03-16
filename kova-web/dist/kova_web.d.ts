/* tslint:disable */
/* eslint-disable */

/**
 * Web handle for JavaScript to start the app.
 */
export class WebHandle {
    free(): void;
    [Symbol.dispose](): void;
    destroy(): void;
    constructor();
    start(canvas: HTMLCanvasElement): Promise<void>;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_webhandle_free: (a: number, b: number) => void;
    readonly webhandle_destroy: (a: number) => void;
    readonly webhandle_new: () => number;
    readonly webhandle_start: (a: number, b: any) => any;
    readonly wasm_bindgen__closure__destroy__hab678735ae1071d5: (a: number, b: number) => void;
    readonly wasm_bindgen__closure__destroy__h98aae8d924984c1d: (a: number, b: number) => void;
    readonly wasm_bindgen__closure__destroy__hc0a96596f95dccdf: (a: number, b: number) => void;
    readonly wasm_bindgen__closure__destroy__h3f56507075c0be92: (a: number, b: number) => void;
    readonly wasm_bindgen__convert__closures_____invoke__hc993b89db8f2add9: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen__convert__closures_____invoke__hdda57ac0e71e965a: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h33ef6f681b924ac1: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h614065639f14f1f8: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__hbd949333bb843917: (a: number, b: number) => [number, number];
    readonly wasm_bindgen__convert__closures_____invoke__h0c9efd71c48846e0: (a: number, b: number) => number;
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
