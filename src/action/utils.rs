#[inline]
pub(crate) fn relative_zoom_from_pot(zoom: i8) -> f64 {
    if zoom > 0 {(1 << zoom) as f64} else {1.0 / (1<<-zoom) as f64}
}