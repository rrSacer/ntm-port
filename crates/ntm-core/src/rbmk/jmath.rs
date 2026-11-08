//! `java.lang.Math` for the RBMK curves, independent of the platform libm.
//!
//! HotSpot on x86-64 answers `Math.log10`, `Math.pow` and `Math.sin` with
//! Intel's libm stubs. The oracle (`tools/rbmk-oracle`) records their bits.
//! glibc rounds `log10` differently in about one argument in five, so these
//! functions evaluate in double-double (about 100 bits) and round once: that
//! reproduces `log10` in every probed case (0 of 4500 differ from the JDK).
//! `pow(E, y)`, `sin` and `cos` agree but for roughly 0.1-0.2% of arguments:
//! the stubs are not correctly rounded there, and their algorithm is not
//! available (glibc misses a similar 0.22% of `sin`). That residual (one ulp,
//! on the last bit of a result) is recorded in `docs/rbmk-core.md`.
//!
//! Arguments outside the range the reactor can produce fall back to the
//! platform: `sin`/`cos` beyond 1e5 radians, and results below the normal
//! range of `pow`.

#[derive(Clone, Copy, Debug)]
struct Dd {
    hi: f64,
    lo: f64,
}

const LN2: Dd = Dd { hi: 0.6931471805599453, lo: 2.3190468138462996e-17 };
const LN10: Dd = Dd { hi: 2.302585092994046, lo: -2.1707562233822494e-16 };
/// `ln(Math.E)` with `Math.E` the double 2.718281828459045.
const LN_E: Dd = Dd { hi: 1.0, lo: -5.318237706605891e-17 };
const PIO2: [f64; 3] = [1.5707963267948966, 6.123233995736766e-17, -1.4973849048591698e-33];

fn two_sum(a: f64, b: f64) -> Dd {
    let s = a + b;
    let bb = s - a;
    Dd { hi: s, lo: (a - (s - bb)) + (b - bb) }
}

fn quick_two_sum(a: f64, b: f64) -> Dd {
    let s = a + b;
    Dd { hi: s, lo: b - (s - a) }
}

fn two_prod(a: f64, b: f64) -> Dd {
    let p = a * b;
    Dd { hi: p, lo: a.mul_add(b, -p) }
}

impl Dd {
    fn from(a: f64) -> Dd {
        Dd { hi: a, lo: 0.0 }
    }

    fn add(self, b: Dd) -> Dd {
        let s = two_sum(self.hi, b.hi);
        let t = two_sum(self.lo, b.lo);
        let e = quick_two_sum(s.hi, s.lo + t.hi);
        quick_two_sum(e.hi, e.lo + t.lo)
    }

    fn neg(self) -> Dd {
        Dd { hi: -self.hi, lo: -self.lo }
    }

    fn sub(self, b: Dd) -> Dd {
        self.add(b.neg())
    }

    fn mul(self, b: Dd) -> Dd {
        let p = two_prod(self.hi, b.hi);
        quick_two_sum(p.hi, p.lo + (self.hi * b.lo + self.lo * b.hi))
    }

    fn mul_f(self, b: f64) -> Dd {
        let p = two_prod(self.hi, b);
        quick_two_sum(p.hi, p.lo + self.lo * b)
    }

    fn div(self, b: Dd) -> Dd {
        let q1 = self.hi / b.hi;
        let r = self.sub(b.mul_f(q1));
        let q2 = r.hi / b.hi;
        let r = r.sub(b.mul_f(q2));
        let q3 = r.hi / b.hi;
        let q = quick_two_sum(q1, q2);
        q.add(Dd::from(q3))
    }

    /// Exact scaling by 2^k.
    fn scale(self, k: i32) -> Dd {
        let f = 2f64.powi(k);
        Dd { hi: self.hi * f, lo: self.lo * f }
    }
}

/// e^a for a in dd, any magnitude the caller has already range-checked.
fn exp_dd(a: Dd) -> Dd {
    let k = (a.hi / LN2.hi).round();
    let r = a.sub(LN2.mul_f(k)).scale(-9);
    // expm1 by Taylor, then (e^r - 1) doubled nine times.
    let mut term = r;
    let mut sum = r;
    for n in 2..=14 {
        term = term.mul(r).div(Dd::from(f64::from(n)));
        sum = sum.add(term);
    }
    let mut s = sum;
    for _ in 0..9 {
        s = s.mul_f(2.0).add(s.mul(s));
    }
    let e = s.add(Dd::from(1.0));
    let k = k as i32;
    // two steps keep 2^k finite for results near the range limits
    e.scale(k / 2).scale(k - k / 2)
}

/// ln x for x > 0 finite, in dd (Halley step from the libm seed).
fn ln_dd(x: f64) -> Dd {
    let y = Dd::from(x.ln());
    let ey = exp_dd(y);
    let xx = Dd::from(x);
    y.add(xx.sub(ey).mul_f(2.0).div(xx.add(ey)))
}

/// `Math.log10`.
pub fn log10(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x.is_infinite() {
        return x;
    }
    let r = ln_dd(x).div(LN10);
    r.hi + r.lo
}

/// `Math.pow(Math.E, y)` (Math.E being the double 2.718281828459045).
pub fn pow_e(y: f64) -> f64 {
    if y.is_nan() {
        return f64::NAN;
    }
    let t = Dd::from(y).mul(LN_E);
    if t.hi > 709.8 {
        return f64::INFINITY;
    }
    if t.hi < -708.0 {
        return std::f64::consts::E.powf(y); // subnormal range: platform
    }
    let r = exp_dd(t);
    r.hi + r.lo
}

/// sin and cos of x via the quadrant reduction; `None` when out of range.
fn sincos_dd(x: f64) -> Option<(Dd, Dd)> {
    if !x.is_finite() || x.abs() > 1e5 {
        return None;
    }
    let n = (x / PIO2[0]).round();
    let mut r = Dd::from(x);
    for p in PIO2 {
        r = r.sub(two_prod(n, p));
    }
    let r2 = r.mul(r);
    // sin r = r (1 - r²/3! (1 - r²/(4·5) (...))), cos likewise, Horner form
    let mut s = Dd::from(1.0);
    let mut c = Dd::from(1.0);
    for k in (1..=15).rev() {
        let (a, b) = (f64::from(2 * k * (2 * k + 1)), f64::from((2 * k - 1) * (2 * k)));
        s = Dd::from(1.0).sub(r2.mul(s).div(Dd::from(a)));
        c = Dd::from(1.0).sub(r2.mul(c).div(Dd::from(b)));
    }
    let (sin_r, cos_r) = (r.mul(s), c);
    Some(match (n as i64).rem_euclid(4) {
        0 => (sin_r, cos_r),
        1 => (cos_r, sin_r.neg()),
        2 => (sin_r.neg(), cos_r.neg()),
        _ => (cos_r.neg(), sin_r),
    })
}

/// `Math.sin`.
pub fn sin(x: f64) -> f64 {
    if x == 0.0 {
        return x;
    }
    match sincos_dd(x) {
        Some((s, _)) => s.hi + s.lo,
        None => x.sin(),
    }
}

/// `Math.cos`.
pub fn cos(x: f64) -> f64 {
    match sincos_dd(x) {
        Some((_, c)) => c.hi + c.lo,
        None => x.cos(),
    }
}

/// `Math.pow(x, y)` for the reactor's cases: `y == 2` is `x * x` (HotSpot
/// special-cases it); a positive finite base goes through double-double; the
/// rest falls back to the platform.
pub fn pow(x: f64, y: f64) -> f64 {
    if y == 2.0 {
        return x * x;
    }
    if y == 0.0 {
        return 1.0;
    }
    if x.is_finite() && x > 0.0 && y.is_finite() {
        let t = ln_dd(x).mul_f(y);
        if t.hi > -700.0 && t.hi < 700.0 {
            let r = exp_dd(t);
            return r.hi + r.lo;
        }
    }
    if x == 0.0 && y > 0.0 {
        return 0.0;
    }
    x.powf(y)
}
