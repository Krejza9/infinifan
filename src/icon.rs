//! Program icon – shared by the window, the tray and build.rs (the .exe file icon), hence no dependencies.

/// Program icon: a simplified SL Infinity (frame with side strips and a glowing ring).
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let mut px = vec![0u8; (size * size * 4) as usize];
    let n = size as f32;
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = ((x as f32 + 0.5) / n * 200.0, (y as f32 + 0.5) / n * 200.0);
            let r = ((fx - 100.0).powi(2) + (fy - 100.0).powi(2)).sqrt();
            let side = fx.min(200.0 - fx);
            let corner = (fy.min(200.0 - fy) + side - 62.0).abs() < 9.0 && side < 40.0;
            let strip = (side < 18.0 && (40.0..160.0).contains(&fy)) || corner;
            let col = if r < 26.0 {
                [12, 13, 16, 255]
            } else if r < 84.0 {
                let t = (r - 26.0) / 58.0;
                [(40.0 + 40.0 * t) as u8, (90.0 - 20.0 * t) as u8, 255, 255]
            } else if strip {
                [34, 211, 238, 255]
            } else {
                [40, 42, 48, 255]
            };
            let i = ((y * size + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&col);
        }
    }
    px
}
