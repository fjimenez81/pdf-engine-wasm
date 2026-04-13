use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use pdf_writer::{
    Chunk, Content, Filter, Finish, Name, Pdf, Rect, Ref, Str,
    types::{LineCapStyle, LineJoinStyle},
};

#[wasm_bindgen]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

#[wasm_bindgen]
pub struct ResizeResult {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[wasm_bindgen]
pub fn textbox_hit_handle(
    bx: f64, by: f64, width: f64, height: f64,
    px: f64, py: f64, handle_size: f64,
) -> u8 {
    let cx = bx + width / 2.0;
    let cy = by + height / 2.0;
    let r = handle_size / 2.0;

    let hits = |hx: f64, hy: f64| {
        px >= hx - r && px <= hx + r && py >= hy - r && py <= hy + r
    };

    if hits(bx,          by)           { return 1; }
    if hits(cx,          by)           { return 2; }
    if hits(bx + width,  by)           { return 3; }
    if hits(bx + width,  cy)           { return 4; }
    if hits(bx + width,  by + height)  { return 5; }
    if hits(cx,          by + height)  { return 6; }
    if hits(bx,          by + height)  { return 7; }
    if hits(bx,          cy)           { return 8; }
    0
}

#[wasm_bindgen]
pub fn textbox_resize(
    bx: f64, by: f64, width: f64, height: f64,
    handle: u8, mx: f64, my: f64,
    min_w: f64, min_h: f64,
    canvas_w: f64, canvas_h: f64,
) -> ResizeResult {
    let mut x = bx;
    let mut y = by;
    let mut w = width;
    let mut h = height;
    let right  = bx + width;
    let bottom = by + height;

    match handle {
        1 => { x = mx.min(right - min_w);  y = my.min(bottom - min_h); w = right - x;  h = bottom - y; }
        2 => { y = my.min(bottom - min_h); h = bottom - y; }
        3 => { y = my.min(bottom - min_h); w = (mx - bx).max(min_w);   h = bottom - y; }
        4 => { w = (mx - bx).max(min_w); }
        5 => { w = (mx - bx).max(min_w);   h = (my - by).max(min_h); }
        6 => { h = (my - by).max(min_h); }
        7 => { x = mx.min(right - min_w);  w = right - x; h = (my - by).max(min_h); }
        8 => { x = mx.min(right - min_w);  w = right - x; }
        _ => {}
    }

    x = x.max(0.0);
    y = y.max(0.0);
    w = w.min(canvas_w - x);
    h = h.min(canvas_h - y);

    ResizeResult { x, y, width: w, height: h }
}

#[wasm_bindgen]
pub fn circle_contains(cx: f64, cy: f64, radius: f64, px: f64, py: f64) -> bool {
    let dx = cx - px;
    let dy = cy - py;
    (dx * dx + dy * dy).sqrt() <= radius
}

#[wasm_bindgen]
pub fn circle_move_to(cx: f64, cy: f64, radius: f64, nx: f64, ny: f64) -> Vec2 {
    Vec2 {
        x: nx.clamp(radius, 500.0 - radius),
        y: ny.clamp(radius, 500.0 - radius),
    }
}

#[wasm_bindgen]
pub fn textbox_contains(bx: f64, by: f64, width: f64, height: f64, px: f64, py: f64) -> bool {
    px >= bx && px <= bx + width && py >= by && py <= by + height
}

#[wasm_bindgen]
pub fn textbox_clamp(nx: f64, ny: f64, width: f64, height: f64, canvas_w: f64, canvas_h: f64) -> Vec2 {
    Vec2 {
        x: nx.clamp(0.0, canvas_w - width),
        y: ny.clamp(0.0, canvas_h - height),
    }
}

// ── Nivel 3: Float64Array compartido ──────────────────────────────────────
//
// Layout del buffer por objeto (8 f64 = 64 bytes cada uno):
// [0] kind   — 0=circle, 1=text
// [1] x
// [2] y
// [3] w      — radio si circle, ancho si text
// [4] h      — 0 si circle, alto si text
// [5] unused
// [6] unused
// [7] unused

const STRIDE: usize = 8;

// Hit test sobre todo el buffer — devuelve el índice del objeto golpeado
// o usize::MAX si ninguno. Itera al revés (z-order).
#[wasm_bindgen]
pub fn buffer_hit_test(
    buf: &[f64],
    count: usize,
    px: f64,
    py: f64,
    handle_size: f64,
) -> u32 {
    let mut i = count;
    while i > 0 {
        i -= 1;
        let base = i * STRIDE;
        let kind = buf[base] as u8;
        let x    = buf[base + 1];
        let y    = buf[base + 2];
        let w    = buf[base + 3];
        let h    = buf[base + 4];

        if kind == 0 {
            // Círculo
            let dx = x - px;
            let dy = y - py;
            if (dx * dx + dy * dy).sqrt() <= w {
                return i as u32;
            }
        } else {
            // TextBox — primero handles, luego interior
            let handle = textbox_hit_handle(x, y, w, h, px, py, handle_size);
            if handle != 0 || textbox_contains(x, y, w, h, px, py) {
                return i as u32;
            }
        }
    }
    u32::MAX
}

// Devuelve el handle activo para un textbox concreto (0 si ninguno)
#[wasm_bindgen]
pub fn buffer_hit_handle(
    buf: &[f64],
    idx: usize,
    px: f64,
    py: f64,
    handle_size: f64,
) -> u8 {
    let base = idx * STRIDE;
    let x = buf[base + 1];
    let y = buf[base + 2];
    let w = buf[base + 3];
    let h = buf[base + 4];
    textbox_hit_handle(x, y, w, h, px, py, handle_size)
}

// Mueve un objeto en el buffer — 1 sola llamada WASM por frame
#[wasm_bindgen]
pub fn buffer_move(
    buf: &mut [f64],
    idx: usize,
    nx: f64,
    ny: f64,
    canvas_w: f64,
    canvas_h: f64,
) {
    let base = idx * STRIDE;
    let kind = buf[base] as u8;
    let w    = buf[base + 3];
    let h    = buf[base + 4];

    if kind == 0 {
        // Círculo — clamp con radio
        buf[base + 1] = nx.clamp(w, canvas_w - w);
        buf[base + 2] = ny.clamp(w, canvas_h - w);
    } else {
        // TextBox — clamp con dimensiones
        buf[base + 1] = nx.clamp(0.0, canvas_w - w);
        buf[base + 2] = ny.clamp(0.0, canvas_h - h);
    }
}

// Resize de un textbox en el buffer
#[wasm_bindgen]
pub fn buffer_resize(
    buf: &mut [f64],
    idx: usize,
    handle: u8,
    mx: f64,
    my: f64,
    min_w: f64,
    min_h: f64,
    canvas_w: f64,
    canvas_h: f64,
) {
    let base = idx * STRIDE;
    let bx = buf[base + 1];
    let by = buf[base + 2];
    let bw = buf[base + 3];
    let bh = buf[base + 4];

    let r = textbox_resize(bx, by, bw, bh, handle, mx, my, min_w, min_h, canvas_w, canvas_h);

    buf[base + 1] = r.x;
    buf[base + 2] = r.y;
    buf[base + 3] = r.width;
    buf[base + 4] = r.height;
}

// Mueve un objeto al final del buffer (z-order — trae al frente)
// Escribe el índice nuevo en la posición destino y compacta
#[wasm_bindgen]
pub fn buffer_bring_to_front(buf: &mut [f64], idx: usize, count: usize) {
    if idx >= count - 1 { return; }
    let base = idx * STRIDE;
    let last = (count - 1) * STRIDE;

    // Guarda el objeto
    let mut tmp = [0f64; 8];
    tmp.copy_from_slice(&buf[base..base + STRIDE]);

    // Desplaza todo lo que está por encima una posición hacia abajo
    for i in idx..count - 1 {
        let src = (i + 1) * STRIDE;
        let dst = i * STRIDE;
        buf.copy_within(src..src + STRIDE, dst);
    }

    // Pone el objeto al final
    buf[last..last + STRIDE].copy_from_slice(&tmp);
}



// ── Tipos de página estándar ───────────────────────────────────────────────

#[wasm_bindgen]
pub fn page_size_pts(format: &str, orientation: &str) -> Vec<f64> {
    let (w, h) = match format {
        "A3"     => (841.89, 1190.55),
        "A4"     => (595.28,  841.89),
        "A5"     => (419.53,  595.28),
        "Letter" => (612.00,  792.00),
        "Legal"  => (612.00, 1008.00),
        _        => (595.28,  841.89), // A4 por defecto
    };
    if orientation == "landscape" { vec![h, w] } else { vec![w, h] }
}

// ── Conversión px → pt ────────────────────────────────────────────────────
// PDF usa pt (1 pt = 1/72 inch). Asumimos 96 DPI para pantalla.
// factor = 72.0 / 96.0 = 0.75

#[wasm_bindgen]
pub fn px_to_pt(px: f64) -> f64 {
    px * 0.75
}

// ═══════════════════════════════════════════════════════════════════════════
// PÁGINA — tamaños estándar en puntos
// ═══════════════════════════════════════════════════════════════════════════

fn page_pts(format: &str, orientation: &str) -> (f32, f32) {
    let (w, h) = match format {
        "A3"     => (841.89_f32, 1190.55_f32),
        "A4"     => (595.28,      841.89),
        "A5"     => (419.53,      595.28),
        "Letter" => (612.00,      792.00),
        "Legal"  => (612.00,     1008.00),
        _        => (595.28,      841.89),
    };
    if orientation == "landscape" { (h, w) } else { (w, h) }
}

// ═══════════════════════════════════════════════════════════════════════════
// JSON — serializa el buffer a template JSON
// ═══════════════════════════════════════════════════════════════════════════

#[wasm_bindgen]
pub fn buffer_to_template_json(
    buf: &[f64],
    count: usize,
    canvas_w_px: f64,
    canvas_h_px: f64,
    page_format: &str,
    orientation: &str,
) -> String {
    let (pw, ph) = page_pts(page_format, orientation);
    let pw = pw as f64;
    let ph = ph as f64;
    let sx = pw / canvas_w_px;
    let sy = ph / canvas_h_px;

    let mut objects = Vec::with_capacity(count);

    for i in 0..count {
        let base = i * STRIDE;
        let kind = buf[base] as u8;
        let x    = buf[base + 1];
        let y    = buf[base + 2];
        let w    = buf[base + 3];
        let h    = buf[base + 4];

        // PDF origin = bottom-left → invertir Y
        let obj = match kind {
            0 => {
                let cx_pt = x * sx;
                let cy_pt = ph - y * sy;
                let r_pt  = w * sx;
                format!(
                    r#"{{"id":{i},"kind":"circle","cx_pt":{cx_pt:.2},"cy_pt":{cy_pt:.2},"radius_pt":{r_pt:.2},"fill":[0.204,0.596,0.859],"stroke":[0.161,0.502,0.725],"stroke_width":1.5}}"#
                )
            }
            1 => {
                let x_pt = x * sx;
                let y_pt = ph - (y + h) * sy; // esquina inferior en PDF
                let w_pt = w * sx;
                let h_pt = h * sy;
                format!(
                    r#"{{"id":{i},"kind":"textbox","x_pt":{x_pt:.2},"y_pt":{y_pt:.2},"width_pt":{w_pt:.2},"height_pt":{h_pt:.2},"content":"","font_size":12}}"#
                )
            }
            _ => continue,
        };
        objects.push(obj);
    }

    format!(
        r#"{{"version":1,"page":{{"format":"{page_format}","width_pt":{pw:.2},"height_pt":{ph:.2},"orientation":"{orientation}"}},"canvas":{{"width_px":{canvas_w_px},"height_px":{canvas_h_px}}},"objects":[{}]}}"#,
        objects.join(",")
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// PDF — genera el PDF desde el JSON del template
// Devuelve los bytes del PDF como Vec<u8> → JS lo recibe como Uint8Array
// ═══════════════════════════════════════════════════════════════════════════

// Referencias internas del PDF (IDs de objetos)
struct Refs {
    catalog:    Ref,
    page_tree:  Ref,
    page:       Ref,
    content:    Ref,
    font:       Ref,
}

impl Refs {
    fn new() -> Self {
        Self {
            catalog:   Ref::new(1),
            page_tree: Ref::new(2),
            page:      Ref::new(3),
            content:   Ref::new(4),
            font:      Ref::new(5),
        }
    }
}

// Aproximación de círculo con 4 curvas Bézier cúbicas
// PDF no tiene operador de círculo nativo
fn circle_path(cx: f32, cy: f32, r: f32) -> String {
    let k = 0.5522847498_f32; // constante de aproximación
    let kr = k * r;
    format!(
        "{:.3} {:.3} m \
         {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c \
         {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c \
         {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c \
         {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c \
         h",
        cx, cy + r,
        cx + kr, cy + r, cx + r, cy + kr, cx + r, cy,
        cx + r, cy - kr, cx + kr, cy - r, cx, cy - r,
        cx - kr, cy - r, cx - r, cy - kr, cx - r, cy,
        cx - r, cy + kr, cx - kr, cy + r, cx, cy + r,
    )
}

#[wasm_bindgen]
pub fn json_to_pdf(json: &str) -> Result<Vec<u8>, JsValue> {
    // ── Parseo del JSON ────────────────────────────────────────────────────
    let template: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| JsValue::from_str(&format!("JSON inválido: {e}")))?;

    let page_w = template["page"]["width_pt"]
        .as_f64().unwrap_or(595.28) as f32;
    let page_h = template["page"]["height_pt"]
        .as_f64().unwrap_or(841.89) as f32;

    let objects = template["objects"]
        .as_array()
        .ok_or_else(|| JsValue::from_str("objects no es un array"))?;

    // ── Estructura del PDF ─────────────────────────────────────────────────
    let refs = Refs::new();
    let mut writer = Pdf::new();

    // Catálogo
    writer.catalog(refs.catalog).pages(refs.page_tree);

    // Árbol de páginas
    writer.pages(refs.page_tree).kids([refs.page]).count(1);

    // Página
    let mut page = writer.page(refs.page);
    page.media_box(Rect::new(0.0, 0.0, page_w, page_h));
    page.parent(refs.page_tree);
    page.contents(refs.content);

    // Fuente Helvetica en los recursos de la página
    let mut resources = page.resources();
    resources
        .fonts()
        .pair(Name(b"F1"), refs.font);
    resources.finish();
    page.finish();

    // Fuente estándar (no necesita embedding)
    writer
        .type1_font(refs.font)
        .base_font(Name(b"Helvetica"));

    // ── Contenido — stream de operadores PDF ──────────────────────────────
    let mut ops = String::new();

    // Configuración inicial de líneas
    ops.push_str("1 j\n"); // line join: round
    ops.push_str("1 J\n"); // line cap: round

    for obj in objects {
        match obj["kind"].as_str().unwrap_or("") {

            "circle" => {
                let cx = obj["cx_pt"].as_f64().unwrap_or(0.0) as f32;
                let cy = obj["cy_pt"].as_f64().unwrap_or(0.0) as f32;
                let r  = obj["radius_pt"].as_f64().unwrap_or(10.0) as f32;
                let sw = obj["stroke_width"].as_f64().unwrap_or(1.5) as f32;

                // Color de relleno (RGB normalizado 0-1)
                let fill = obj["fill"].as_array();
                let (fr, fg, fb) = fill.map(|c| (
                    c[0].as_f64().unwrap_or(0.2) as f32,
                    c[1].as_f64().unwrap_or(0.6) as f32,
                    c[2].as_f64().unwrap_or(0.86) as f32,
                )).unwrap_or((0.204, 0.596, 0.859));

                let stroke = obj["stroke"].as_array();
                let (sr, sg, sb) = stroke.map(|c| (
                    c[0].as_f64().unwrap_or(0.16) as f32,
                    c[1].as_f64().unwrap_or(0.5) as f32,
                    c[2].as_f64().unwrap_or(0.72) as f32,
                )).unwrap_or((0.161, 0.502, 0.725));

                ops.push_str(&format!(
                    "q\n\
                     {fr:.3} {fg:.3} {fb:.3} rg\n\
                     {sr:.3} {sg:.3} {sb:.3} RG\n\
                     {sw:.3} w\n\
                     {path}\n\
                     B\n\
                     Q\n",
                    path = circle_path(cx, cy, r),
                ));
            }

            "textbox" => {
                let x    = obj["x_pt"].as_f64().unwrap_or(0.0) as f32;
                let y    = obj["y_pt"].as_f64().unwrap_or(0.0) as f32;
                let w    = obj["width_pt"].as_f64().unwrap_or(100.0) as f32;
                let h    = obj["height_pt"].as_f64().unwrap_or(20.0) as f32;
                let fs   = obj["font_size"].as_f64().unwrap_or(12.0) as f32;
                let text = obj["content"].as_str().unwrap_or("");

                // Borde del textbox (rect con trazo)
                ops.push_str(&format!(
                    "q\n\
                     0.8 0.8 0.8 RG\n\
                     0.5 w\n\
                     {x:.3} {y:.3} {w:.3} {h:.3} re\n\
                     S\n\
                     Q\n"
                ));

                // Texto — solo si tiene contenido
                if !text.is_empty() {
                    // Escapar paréntesis en PDF
                    let safe = text.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
                    let ty = y + h - fs - 4.0; // margen superior de 4pt

                    ops.push_str(&format!(
                        "q\n\
                         BT\n\
                         /F1 {fs:.1} Tf\n\
                         0 0 0 rg\n\
                         {x:.3} {ty:.3} Td\n\
                         ({safe}) Tj\n\
                         ET\n\
                         Q\n"
                    ));
                }
            }

            "line" => {
                let x1 = obj["x1_pt"].as_f64().unwrap_or(0.0) as f32;
                let y1 = obj["y1_pt"].as_f64().unwrap_or(0.0) as f32;
                let x2 = obj["x2_pt"].as_f64().unwrap_or(0.0) as f32;
                let y2 = obj["y2_pt"].as_f64().unwrap_or(0.0) as f32;
                let sw = obj["stroke_width"].as_f64().unwrap_or(1.0) as f32;

                ops.push_str(&format!(
                    "q\n\
                     0 0 0 RG\n\
                     {sw:.3} w\n\
                     {x1:.3} {y1:.3} m\n\
                     {x2:.3} {y2:.3} l\n\
                     S\n\
                     Q\n"
                ));
            }

            _ => {} // tipos futuros (image, etc.) — ignorar sin romper
        }
    }

    // Stream de contenido
    writer.stream(refs.content, ops.as_bytes());

    Ok(writer.finish())
}