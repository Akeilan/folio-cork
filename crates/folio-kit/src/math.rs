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
    /// x * x.
    fn sq(self) -> Self;
}

impl Det for f32 {
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
    fn sq(self) -> f32 {
        self * self
    }
}

impl Det for f64 {
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
    fn sq(self) -> f64 {
        self * self
    }
}
