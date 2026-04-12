use wasm_bindgen::prelude::*;

// Configura el hook de pánico para ver errores de Rust en la consola de Chrome
#[wasm_bindgen(start)]
pub fn main_js() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    Ok(())
}

#[wasm_bindgen]
pub fn procesar_informe(nombre_medico: &str) -> String {
    format!("Motor listo. Dr/Dra. {}, el PC antiguo está bajo control.", nombre_medico)
}