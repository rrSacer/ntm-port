//! `com.hbm.util.function.Function` (reactivity curves) and `BobMathUtil.squirt`.
//!
//! The Java classes keep `div` and `off` as mutable builder state; here they are plain fields.
//! `x` is mapped as `x / div + off` before the curve is applied, exactly like `Function.getX`.

/// `BobMathUtil.squirt`: a square root with the small-argument singularity softened.
pub fn squirt(x: f64) -> f64 {
    (x + 1.0 / ((x + 2.0) * (x + 2.0))).sqrt() - 1.0 / (x + 2.0)
}

/// One reactivity curve. `level` is the multiplier of the source class; `v_off` only exists
/// for the quadratic form.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Curve {
    Linear,
    Sqrt,
    Logarithmic,
    Quadratic { v_off: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Function {
    pub curve: Curve,
    pub level: f64,
    pub div: f64,
    pub off: f64,
}

impl Function {
    const fn new(curve: Curve, level: f64, div: f64, off: f64) -> Self {
        Function { curve, level, div, off }
    }
    /// `new FunctionLinear(level)`
    pub const fn linear(level: f64) -> Self {
        Self::new(Curve::Linear, level, 1.0, 0.0)
    }
    /// `new FunctionSqrt(level)`
    pub const fn sqrt(level: f64) -> Self {
        Self::new(Curve::Sqrt, level, 1.0, 0.0)
    }
    /// `new FunctionSqrtFalling(fall)`: `FunctionSqrt(1 / fall)` with offset `fall * fall`.
    pub fn sqrt_falling(fall: f64) -> Self {
        Self::new(Curve::Sqrt, 1.0 / fall, 1.0, fall * fall)
    }
    /// `new FunctionLogarithmic(level)`: the constructor already applies `withOff(1)`.
    pub const fn logarithmic(level: f64) -> Self {
        Self::new(Curve::Logarithmic, level, 1.0, 1.0)
    }
    /// `new FunctionQuadratic(level, v_off)`
    pub const fn quadratic(level: f64, v_off: f64) -> Self {
        Self::new(Curve::Quadratic { v_off }, level, 1.0, 0.0)
    }
    pub const fn with_div(mut self, div: f64) -> Self {
        self.div = div;
        self
    }
    pub const fn with_off(mut self, off: f64) -> Self {
        self.off = off;
        self
    }

    fn get_x(&self, x: f64) -> f64 {
        x / self.div + self.off
    }

    /// `Function.effonix`
    pub fn effonix(&self, x: f64) -> f64 {
        match self.curve {
            Curve::Linear => self.get_x(x) * self.level,
            Curve::Sqrt => squirt(self.get_x(x)) * self.level,
            Curve::Logarithmic => self.get_x(x).log10() * self.level,
            Curve::Quadratic { v_off } => self.get_x(x) * self.get_x(x) * self.level + v_off,
        }
    }
}

/// Java's `(int) Math.round(x)`: `Math.round` returns a `long` that the cast then wraps to 32 bits.
pub fn java_round_to_int(x: f64) -> i32 {
    let r = if x.is_nan() {
        0i64
    } else {
        let f = x.floor();
        (if x - f >= 0.5 { f + 1.0 } else { f }) as i64
    };
    r as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_matches_java_edge_cases() {
        assert_eq!(java_round_to_int(0.5), 1);
        assert_eq!(java_round_to_int(-0.5), 0);
        assert_eq!(java_round_to_int(0.49999999999999994), 0);
        assert_eq!(java_round_to_int(2.5), 3);
        assert_eq!(java_round_to_int(-2.5), -2);
        assert_eq!(java_round_to_int(f64::NAN), 0);
        // the long result wraps when narrowed to int
        assert_eq!(java_round_to_int(4_294_967_296.0 + 7.0), 7);
    }
}
