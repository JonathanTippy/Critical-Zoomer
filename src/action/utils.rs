#[inline]
pub(crate) fn zoom_from_pot(zoom: i64) -> f64 {
    if zoom > 0 {(1 << zoom) as f64} else {1.0 / (1<<-zoom) as f64}
}

#[inline]
pub(crate) fn signed_shift(input: i32, shift: i64) -> i32 {
    (input << ((shift + (shift.abs()))>>1)) >> (-((shift - (shift.abs()))>>1))
    /*if shift >= 0 {
        input << shift
    } else {
        input >> (-shift)
    }*/
}

/*#[inline]
pub(crate) fn shift_signed_assume_left(input: i32, shift: i64) -> i32 {
    if shift >= 0 {
        input
    }
}*/