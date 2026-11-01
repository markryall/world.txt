//! Tiny deterministic randomness: no dependency, no API churn.

pub fn hash(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

pub fn hash2(seed: u64, x: i64, y: i64) -> u64 {
    hash(seed ^ hash((x as u64) ^ hash(y as u64).rotate_left(17)))
}

/// 0.0..1.0 for a lattice point.
pub fn unit2(seed: u64, x: i64, y: i64) -> f64 {
    (hash2(seed, x, y) >> 11) as f64 / (1u64 << 53) as f64
}

/// Pick something stable for a place, so descriptions don't flicker.
pub fn pick<'a, T>(items: &'a [T], seed: u64, x: i64, y: i64) -> &'a T {
    &items[(hash2(seed, x, y) % items.len() as u64) as usize]
}

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(hash(seed) | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 = hash(self.0);
        self.0
    }
    pub fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn chance(&mut self, p: f64) -> bool {
        self.f() < p
    }
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.f() * (hi - lo)
    }
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next() % items.len() as u64) as usize]
    }
}

/// Smooth value noise in roughly 0..1.
pub fn noise(seed: u64, x: f64, y: f64) -> f64 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let s = |t: f64| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(fx), s(fy));
    let (ix, iy) = (x0 as i64, y0 as i64);
    let a = unit2(seed, ix, iy);
    let b = unit2(seed, ix + 1, iy);
    let c = unit2(seed, ix, iy + 1);
    let d = unit2(seed, ix + 1, iy + 1);
    let top = a + (b - a) * sx;
    let bot = c + (d - c) * sx;
    top + (bot - top) * sy
}

/// Layered noise.
pub fn fbm(seed: u64, x: f64, y: f64) -> f64 {
    let mut total = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    for i in 0..4 {
        total += amp * noise(seed.wrapping_add(i), x * freq, y * freq);
        amp *= 0.5;
        freq *= 2.0;
    }
    total / 0.9375
}
