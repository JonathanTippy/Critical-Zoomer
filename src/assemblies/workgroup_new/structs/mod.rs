pub mod sparse_views;
pub mod mandelbrotable;

use std::cmp::Ordering;
use crate::assemblies::structs::*;
use std::collections::*;
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::intexp::*;

#[derive(PartialEq, Clone, Debug)]

pub struct SparseView<T> {
    stencil: PointStencil
    , points: Vec<(T, u8, (usize, usize))>
    , map: HashMap<(usize, usize), usize>
}


impl<T: Copy + Clone> From<View<T>> for SparseView<T> {
    fn from(input: View<T>) -> SparseView<T> {
        let mut returned = SparseView::new(input.stencil);
        for i in 0..input.data.len() {
            if input.bitmap[i] != 0 {
                let value = input.data[i];
                let align = input.bitmap[i];
                returned.insert_with_align((value, align, returned.stencil.seat_and_row(i)));
            }
        }
        returned
    }
}

enum SerialWorkUpdate {
    NewStencil {
        stencil: PointStencil
    }
    ,
    PointDone {}
}

/*struct ActivePoint<T: Mandelbrotable> {
    z: (T, T)
}*/


