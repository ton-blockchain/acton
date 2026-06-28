use acton_source_trace::{BuildCompiledSourceTraceRequest, build_compiled_source_trace_response};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_source_trace(payload: JsValue) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let request: BuildCompiledSourceTraceRequest =
        serde_wasm_bindgen::from_value(payload).map_err(js_error)?;
    let response = build_compiled_source_trace_response(request).map_err(js_error)?;

    serde_wasm_bindgen::to_value(&response).map_err(js_error)
}

fn js_error(error: impl ToString) -> JsValue {
    JsValue::from_str(&error.to_string())
}
