use lazy_static::lazy_static;
use wasm_bindgen::prelude::*;

use std::sync::RwLock;
use std::sync::Mutex;
use std::sync::Arc;
use std::collections::HashMap;

use super::*;

lazy_static! {
    pub static ref FRAMES: Arc<RwLock<HashMap<u32, Frame>>> = Arc::new(RwLock::new(HashMap::new()));
    pub static ref LEFTOVER_SEGS: Arc<Mutex<Vec<Segment>>> = Arc::new(Mutex::new(Vec::new()));
    pub static ref LEFTOVER_BYTES: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
}

#[wasm_bindgen]
#[no_mangle]
pub fn try_parse(data: &[u8]) {
    println!("data len: {}", data.len());
    
    let mut lock = LEFTOVER_BYTES.lock().unwrap();
    let mut segs_lock = LEFTOVER_SEGS.lock().unwrap();
    lock.append(&mut data.to_vec());

    let mut data_bytes = lock.as_slice();

    loop {
        if let Ok((leftover, segment)) = parse_segment(data_bytes) {
            segs_lock.push(segment);

            if leftover.is_empty() {
                break;
            }

            data_bytes = leftover;
        } else {
            break;
        }
    }

    *lock = data_bytes.to_vec();

    let mut frames = FRAMES.write().unwrap();
    while let Some(frame) = try_take_frame(&mut segs_lock) {
        frames.insert(frame.pts(), frame);
    }
}

#[wasm_bindgen]
#[no_mangle]
pub fn render(pts: u32) -> Option<u8> {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::Clamped;

    let frames = FRAMES.read().unwrap();
    let to_render = frames.get(&pts)?;
    let image = to_render.get_pixels()?;

    let document = web_sys::window().unwrap().document().unwrap();
    let canvas = document.get_element_by_id("canvas").unwrap();
    let canvas: web_sys::HtmlCanvasElement = canvas
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| ())
        .unwrap();

    let context = canvas
        .get_context("2d")
        .unwrap()
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .unwrap();

    let html_image = web_sys::ImageData::new_with_u8_clamped_array_and_sh(Clamped(&image), image.width(), image.height()).unwrap();

    context.put_image_data(&html_image, to_render.image_x()? as f64, to_render.image_y()? as f64).unwrap();

    Some(0)
}
