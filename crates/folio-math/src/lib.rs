//! Deterministic maths (X3-12): sine, cosine and the rest from one portable implementation
//! (the `libm` crate) instead of each system's C library, so geometry, exports and saved files
//! come out the same to the bit on Linux, Windows and macOS, on x86, ARM and RISC-V.
//! Folio code calls these (`a.dsin()`), never `f32::sin`; tools/check-portability.sh checks.

pub trait Det: Sized {
    fn dsin(self) -> Self;
    fn dcos(self) -> Self;
    fn dtan(self) -> Self;
    fn datan2(self, x: Self) -> Self;
    fn dln(self) -> Self;
    fn dexp(self) -> Self;
    fn dpow(self, e: Self) -> Self;
    fn dacos(self) -> Self;
    fn dcbrt(self) -> Self;
    /// Integer power by repeated multiplication, in a fixed order.
    fn dpowi(self, n: i32) -> Self;
    fn dasin(self) -> Self;
    fn datan(self) -> Self;
    fn dsinh(self) -> Self;
    fn dcosh(self) -> Self;
    fn dtanh(self) -> Self;
    fn dlog10(self) -> Self;
    fn dlog2(self) -> Self;
    /// The logarithm of self to the given base.
    fn dlog(self, base: Self) -> Self;
    fn dexp2(self) -> Self;
    fn dhypot(self, other: Self) -> Self;
    fn dsqrt(self) -> Self;
    /// x * x.
    fn sq(self) -> Self;
}

impl Det for f32 {
    fn dacos(self) -> f32 {
        libm::acosf(self)
    }
    fn dcbrt(self) -> f32 {
        libm::cbrtf(self)
    }
    fn dpowi(self, n: i32) -> f32 {
        let mut r: f32 = 1.0;
        for _ in 0..n.unsigned_abs() {
            r *= self;
        }
        if n < 0 {
            1.0 / r
        } else {
            r
        }
    }
    fn dsin(self) -> f32 {
        libm::sinf(self)
    }
    fn dcos(self) -> f32 {
        libm::cosf(self)
    }
    fn dtan(self) -> f32 {
        libm::tanf(self)
    }
    fn datan2(self, x: f32) -> f32 {
        libm::atan2f(self, x)
    }
    fn dln(self) -> f32 {
        libm::logf(self)
    }
    fn dexp(self) -> f32 {
        libm::expf(self)
    }
    fn dpow(self, e: f32) -> f32 {
        libm::powf(self, e)
    }
    fn dasin(self) -> f32 {
        libm::asinf(self)
    }
    fn datan(self) -> f32 {
        libm::atanf(self)
    }
    fn dsinh(self) -> f32 {
        libm::sinhf(self)
    }
    fn dcosh(self) -> f32 {
        libm::coshf(self)
    }
    fn dtanh(self) -> f32 {
        libm::tanhf(self)
    }
    fn dlog10(self) -> f32 {
        libm::log10f(self)
    }
    fn dlog2(self) -> f32 {
        libm::log2f(self)
    }
    fn dlog(self, base: f32) -> f32 {
        libm::logf(self) / libm::logf(base)
    }
    fn dexp2(self) -> f32 {
        libm::exp2f(self)
    }
    fn dhypot(self, other: f32) -> f32 {
        libm::hypotf(self, other)
    }
    fn dsqrt(self) -> f32 {
        libm::sqrtf(self)
    }
    fn sq(self) -> f32 {
        self * self
    }
}

impl Det for f64 {
    fn dacos(self) -> f64 {
        libm::acos(self)
    }
    fn dcbrt(self) -> f64 {
        libm::cbrt(self)
    }
    fn dpowi(self, n: i32) -> f64 {
        let mut r: f64 = 1.0;
        for _ in 0..n.unsigned_abs() {
            r *= self;
        }
        if n < 0 {
            1.0 / r
        } else {
            r
        }
    }
    fn dsin(self) -> f64 {
        libm::sin(self)
    }
    fn dcos(self) -> f64 {
        libm::cos(self)
    }
    fn dtan(self) -> f64 {
        libm::tan(self)
    }
    fn datan2(self, x: f64) -> f64 {
        libm::atan2(self, x)
    }
    fn dln(self) -> f64 {
        libm::log(self)
    }
    fn dexp(self) -> f64 {
        libm::exp(self)
    }
    fn dpow(self, e: f64) -> f64 {
        libm::pow(self, e)
    }
    fn dasin(self) -> f64 {
        libm::asin(self)
    }
    fn datan(self) -> f64 {
        libm::atan(self)
    }
    fn dsinh(self) -> f64 {
        libm::sinh(self)
    }
    fn dcosh(self) -> f64 {
        libm::cosh(self)
    }
    fn dtanh(self) -> f64 {
        libm::tanh(self)
    }
    fn dlog10(self) -> f64 {
        libm::log10(self)
    }
    fn dlog2(self) -> f64 {
        libm::log2(self)
    }
    fn dlog(self, base: f64) -> f64 {
        libm::log(self) / libm::log(base)
    }
    fn dexp2(self) -> f64 {
        libm::exp2(self)
    }
    fn dhypot(self, other: f64) -> f64 {
        libm::hypot(self, other)
    }
    fn dsqrt(self) -> f64 {
        libm::sqrt(self)
    }
    fn sq(self) -> f64 {
        self * self
    }
}
