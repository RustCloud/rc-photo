//! What the face models take and give, as plain math with no calls to the
//! server: a picture as planes of floats, the detector's input and output,
//! faces turned to ArcFace's template, vectors made small.
//!
//! The server only runs the models (`use-onnx`); everything around them —
//! sizes, crops, turns, reading the outputs — is here.

/// A picture as three planes (R, G, B) of values 0–255, row by row.
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl Picture {
    /// From an f32 NCHW tensor's little-endian bytes, `[1, 3, height, width]`.
    pub fn from_tensor(width: usize, height: usize, bytes: &[u8]) -> Option<Self> {
        if bytes.len() != width * height * 3 * 4 {
            return None;
        }
        let data = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
        Some(Self { width, height, data })
    }

    fn at(&self, channel: usize, x: usize, y: usize) -> f32 {
        self.data[(channel * self.height + y) * self.width + x]
    }

    /// The value at a point between pixels (bilinear); none outside.
    pub fn sample(&self, channel: usize, x: f32, y: f32) -> Option<f32> {
        if !(x >= 0.0 && y >= 0.0 && x <= (self.width - 1) as f32 && y <= (self.height - 1) as f32) {
            return None;
        }
        let (x0, y0) = (x as usize, y as usize);
        let (x1, y1) = ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let top = self.at(channel, x0, y0) * (1.0 - fx) + self.at(channel, x1, y0) * fx;
        let bottom = self.at(channel, x0, y1) * (1.0 - fx) + self.at(channel, x1, y1) * fx;
        Some(top * (1.0 - fy) + bottom * fy)
    }

    /// The average of the pixels in a rectangle (for making it smaller).
    fn average(&self, channel: usize, x0: usize, y0: usize, x1: usize, y1: usize) -> f32 {
        let mut sum = 0.0;
        for y in y0..y1 {
            let row = (channel * self.height + y) * self.width;
            sum += self.data[row + x0..row + x1].iter().sum::<f32>();
        }
        sum / ((x1 - x0) * (y1 - y0)) as f32
    }
}

/// Fits `width × height` into `side × side`, never larger: the size to ask
/// the server for.
pub fn fit_inside(width: u32, height: u32, side: u32) -> (u32, u32) {
    let scale = (side as f32 / width.max(height) as f32).min(1.0);
    (((width as f32 * scale).round() as u32).max(1), ((height as f32 * scale).round() as u32).max(1))
}

/// The detector's input: the picture at the top left of a black
/// `size × size` square, values 0–1; and the scale it was made smaller by.
pub fn letterbox(picture: &Picture, size: usize) -> (Vec<f32>, f32) {
    let scale = (size as f32 / picture.width as f32).min(size as f32 / picture.height as f32);
    let width = ((picture.width as f32 * scale).round() as usize).clamp(1, size);
    let height = ((picture.height as f32 * scale).round() as usize).clamp(1, size);
    let mut data = vec![0.0; 3 * size * size];
    for channel in 0..3 {
        for y in 0..height {
            for x in 0..width {
                let value = if scale < 1.0 {
                    // Each pixel of the square averages the pixels it covers.
                    let x0 = (x as f32 / scale) as usize;
                    let y0 = (y as f32 / scale) as usize;
                    let x1 = (((x + 1) as f32 / scale) as usize).clamp(x0 + 1, picture.width);
                    let y1 = (((y + 1) as f32 / scale) as usize).clamp(y0 + 1, picture.height);
                    picture.average(channel, x0.min(picture.width - 1), y0.min(picture.height - 1), x1, y1)
                } else {
                    picture.sample(channel, x as f32 / scale, y as f32 / scale).unwrap_or(0.0)
                };
                data[(channel * size + y) * size + x] = value / 255.0;
            }
        }
    }
    (data, scale)
}

/// A face the detector found, in the picture's pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// Left, top, right, bottom.
    pub bbox: [f32; 4],
    pub score: f32,
    /// The eyes (left, right as seen), the nose, the corners of the mouth.
    pub points: [[f32; 2]; 5],
}

impl Found {
    pub fn width(&self) -> f32 {
        self.bbox[2] - self.bbox[0]
    }
}

/// Reads a YOLO face model's output, `[1, 20, anchors]` (or `[1, anchors,
/// 20]`): per anchor the centre, width and height of a box, its score, then
/// x, y and visibility of 5 points. Boxes are in the letterboxed input;
/// `scale` brings them back to the picture.
pub fn decode_yolo(output: &[f32], shape: &[u32], scale: f32, min_score: f32) -> Vec<Found> {
    let [_, a, b] = shape else { return Vec::new() };
    let (a, b) = (*a as usize, *b as usize);
    // 20 channels (more for models with more points): the other side is anchors.
    let transposed = if a == 20 || b == 20 { b == 20 && a != 20 } else { a > b };
    let (channels, anchors) = if transposed { (b, a) } else { (a, b) };
    if channels < 20 || output.len() != channels * anchors {
        return Vec::new();
    }
    let at = |c: usize, i: usize| if transposed { output[i * channels + c] } else { output[c * anchors + i] };
    let mut found = Vec::new();
    for i in 0..anchors {
        let score = at(4, i);
        if score < min_score {
            continue;
        }
        let (cx, cy, w, h) = (at(0, i), at(1, i), at(2, i), at(3, i));
        let bbox = [(cx - w / 2.0) / scale, (cy - h / 2.0) / scale, (cx + w / 2.0) / scale, (cy + h / 2.0) / scale];
        let mut points = [[0.0; 2]; 5];
        for (k, point) in points.iter_mut().enumerate() {
            *point = [at(5 + k * 3, i) / scale, at(6 + k * 3, i) / scale];
        }
        found.push(Found { bbox, score, points });
    }
    nms(found, 0.45)
}

fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let w = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let h = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    let both = w * h;
    let union = (a[2] - a[0]) * (a[3] - a[1]) + (b[2] - b[0]) * (b[3] - b[1]) - both;
    if union > 0.0 {
        both / union
    } else {
        0.0
    }
}

/// Keeps the best of boxes that overlap more than `limit`.
pub fn nms(mut found: Vec<Found>, limit: f32) -> Vec<Found> {
    found.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Found> = Vec::new();
    for face in found {
        if kept.iter().all(|k| iou(&k.bbox, &face.bbox) < limit) {
            kept.push(face);
        }
    }
    kept
}

/// ArcFace's side.
pub const FACE_SIDE: usize = 112;

/// Where ArcFace expects the 5 points in its 112 × 112 input.
const TEMPLATE: [[f32; 2]; 5] =
    [[38.2946, 51.6963], [73.5318, 51.5014], [56.0252, 71.7366], [41.5493, 92.3655], [70.7299, 92.2041]];

/// The turn, scale and shift that best put the template's points on
/// `points`: `[a, b, tx, ty]` maps (x, y) of the input to
/// (a·x − b·y + tx, b·x + a·y + ty) in the picture (least squares).
pub fn similarity(points: &[[f32; 2]; 5]) -> [f32; 4] {
    let mean = |set: &[[f32; 2]; 5], i: usize| set.iter().map(|p| p[i]).sum::<f32>() / 5.0;
    let (px, py) = (mean(&TEMPLATE, 0), mean(&TEMPLATE, 1));
    let (qx, qy) = (mean(points, 0), mean(points, 1));
    let (mut along, mut across, mut norm) = (0.0, 0.0, 0.0);
    for (p, q) in TEMPLATE.iter().zip(points) {
        let (x, y) = (p[0] - px, p[1] - py);
        let (u, v) = (q[0] - qx, q[1] - qy);
        along += x * u + y * v;
        across += x * v - y * u;
        norm += x * x + y * y;
    }
    let (a, b) = (along / norm, across / norm);
    [a, b, qx - (a * px - b * py), qy - (b * px + a * py)]
}

/// ArcFace's input for one face: turned and scaled so its points fall on
/// the template, `(v − 127.5) / 127.5` per colour, black outside the picture;
/// appended to `out` as three 112 × 112 planes.
pub fn aligned_face(picture: &Picture, points: &[[f32; 2]; 5], out: &mut Vec<f32>) {
    let [a, b, tx, ty] = similarity(points);
    for channel in 0..3 {
        for y in 0..FACE_SIDE {
            for x in 0..FACE_SIDE {
                let (fx, fy) = (x as f32, y as f32);
                let value = picture.sample(channel, a * fx - b * fy + tx, b * fx + a * fy + ty).unwrap_or(0.0);
                out.push((value - 127.5) / 127.5);
            }
        }
    }
}

/// Scales a vector to length 1.
pub fn normalize(vector: &mut [f32]) {
    let length = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length > 0.0 {
        vector.iter_mut().for_each(|v| *v /= length);
    }
}

/// A vector as bytes (`i8`), scaled so its largest value is ±127: only its
/// direction matters.
pub fn quantize(vector: &[f32]) -> Vec<u8> {
    let largest = vector.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let scale = if largest > 0.0 { 127.0 / largest } else { 0.0 };
    vector.iter().map(|v| (v * scale).round().clamp(-127.0, 127.0) as i8 as u8).collect()
}

/// Back from [`quantize`], length 1 again.
#[cfg(test)]
fn dequantize(bytes: &[u8]) -> Vec<f32> {
    let mut vector: Vec<f32> = bytes.iter().map(|&b| b as i8 as f32).collect();
    normalize(&mut vector);
    vector
}

/// Cosine similarity of unit vectors.
#[cfg(test)]
fn similar(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(width: usize, height: usize, value: impl Fn(usize, usize, usize) -> f32) -> Picture {
        let mut data = Vec::new();
        for c in 0..3 {
            for y in 0..height {
                for x in 0..width {
                    data.push(value(c, x, y));
                }
            }
        }
        Picture { width, height, data }
    }

    #[test]
    fn sizes_fit_and_never_grow() {
        assert_eq!(fit_inside(4000, 3000, 1600), (1600, 1200));
        assert_eq!(fit_inside(3000, 4000, 1600), (1200, 1600));
        assert_eq!(fit_inside(800, 600, 1600), (800, 600));
    }

    #[test]
    fn tensor_bytes_become_a_picture() {
        let values: Vec<f32> = (0..2 * 1 * 3).map(|n| n as f32).collect();
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        let picture = Picture::from_tensor(2, 1, &bytes).unwrap();
        assert_eq!(picture.at(2, 1, 0), 5.0);
        assert!(Picture::from_tensor(3, 1, &bytes).is_none());
    }

    #[test]
    fn the_letterbox_is_black_past_the_picture() {
        // 1280 × 640 white: the top half of the square is white, the rest black.
        let white = picture(1280, 640, |_, _, _| 255.0);
        let (data, scale) = letterbox(&white, 640);
        assert_eq!(scale, 0.5);
        assert_eq!(data.len(), 3 * 640 * 640);
        assert!((data[100 * 640 + 100] - 1.0).abs() < 1e-6);
        assert_eq!(data[400 * 640 + 100], 0.0);
        // Smaller pictures are stretched up.
        let small = picture(320, 320, |_, x, _| x as f32);
        let (data, scale) = letterbox(&small, 640);
        assert_eq!(scale, 2.0);
        assert!((data[10 * 640 + 200] * 255.0 - 100.0).abs() < 1e-3);
    }

    /// One anchor's 20 channels in a `[1, 20, anchors]` output.
    fn output(anchors: &[[f32; 20]]) -> Vec<f32> {
        let mut out = vec![0.0; 20 * anchors.len()];
        for (i, anchor) in anchors.iter().enumerate() {
            for c in 0..20 {
                out[c * anchors.len() + i] = anchor[c];
            }
        }
        out
    }

    fn anchor(cx: f32, cy: f32, side: f32, score: f32) -> [f32; 20] {
        let mut a = [0.0; 20];
        a[..5].copy_from_slice(&[cx, cy, side, side, score]);
        for k in 0..5 {
            a[5 + k * 3] = cx + k as f32;
            a[6 + k * 3] = cy;
            a[7 + k * 3] = 1.0;
        }
        a
    }

    #[test]
    fn yolo_output_is_read_and_overlaps_are_dropped() {
        let anchors = [
            anchor(100.0, 100.0, 40.0, 0.9),
            anchor(102.0, 101.0, 40.0, 0.8),
            anchor(300.0, 300.0, 20.0, 0.7),
            anchor(500.0, 500.0, 20.0, 0.2),
        ];
        let found = decode_yolo(&output(&anchors), &[1, 20, 4], 0.5, 0.5);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].bbox, [160.0, 160.0, 240.0, 240.0]);
        assert_eq!(found[0].points[1], [202.0, 200.0]);
        assert_eq!(found[1].score, 0.7);
        // The other layout gives the same.
        let mut transposed = vec![0.0; 80];
        for (i, a) in anchors.iter().enumerate() {
            transposed[i * 20..i * 20 + 20].copy_from_slice(a);
        }
        assert_eq!(decode_yolo(&transposed, &[1, 4, 20], 0.5, 0.5), found);
        // A model without points isn't one.
        assert!(decode_yolo(&[0.0; 20], &[1, 5, 4], 0.5, 0.5).is_empty());
    }

    #[test]
    fn the_template_itself_needs_no_change() {
        let [a, b, tx, ty] = similarity(&TEMPLATE);
        assert!((a - 1.0).abs() < 1e-5 && b.abs() < 1e-5 && tx.abs() < 1e-3 && ty.abs() < 1e-3);
    }

    #[test]
    fn a_turned_face_is_turned_back() {
        // The template twice as large, turned 90°, moved by (500, 300).
        let points = TEMPLATE.map(|[x, y]| [-2.0 * y + 500.0, 2.0 * x + 300.0]);
        let [a, b, tx, ty] = similarity(&points);
        assert!(a.abs() < 1e-4 && (b - 2.0).abs() < 1e-4, "{a} {b}");
        assert!((tx - 500.0).abs() < 1e-2 && (ty - 300.0).abs() < 1e-2);
    }

    #[test]
    fn faces_are_cut_out_at_the_template() {
        // Brightness grows to the right: an aligned, unturned face keeps that.
        let ramp = picture(400, 400, |_, x, _| x as f32 / 2.0);
        let points = TEMPLATE.map(|[x, y]| [x + 100.0, y + 100.0]);
        let mut out = Vec::new();
        aligned_face(&ramp, &points, &mut out);
        assert_eq!(out.len(), 3 * FACE_SIDE * FACE_SIDE);
        let value = |x: usize| out[50 * FACE_SIDE + x] * 127.5 + 127.5;
        assert!((value(0) - 50.0).abs() < 0.5 && (value(100) - 100.0).abs() < 0.5);
        // Outside the picture: black.
        let mut out = Vec::new();
        aligned_face(&ramp, &TEMPLATE.map(|[x, y]| [x - 60.0, y]), &mut out);
        assert_eq!(out[50 * FACE_SIDE], -1.0);
    }

    #[test]
    fn small_vectors_stay_close() {
        let mut v: Vec<f32> = (0..512).map(|n| ((n * 37 % 101) as f32) - 50.0).collect();
        normalize(&mut v);
        let back = dequantize(&quantize(&v));
        assert!(similar(&v, &back) > 0.9995, "{}", similar(&v, &back));
    }
}
