use wasm_bindgen::prelude::*;

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