use std::cmp::Ordering;
use crate::assemblies::structs::*;
use std::collections::*;
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::utils::IntExp;

pub struct SparseView<T> {
    stencil: PixelStencil
    , points: Vec<(T, u8, (usize, usize))>
    , map: HashMap<(usize, usize), usize>
}

impl<T: Copy + Clone> SparseView<T> {

    pub fn insert(&mut self, new: (T, (usize, usize))) {

        match self.map.get(&new.1) {
            None => {
                let index = self.points.len();
                self.points.push((new.0, EXACT + EST, new.1));
                self.map.insert(new.1, index);
            }
            , Some(s) => {
                self.points[*s] = (new.0, EXACT + EST, new.1);
            }
        }
    }

    pub fn into(self, fill_value: T) -> View<T> {
        let mut returned = View::new(self.stencil, fill_value);
        for p in self.points {
            let index = returned.stencil.index_trust_input((p.2.0 as isize, p.2.1 as isize));
            returned.data[index] = p.0;
            returned.bitmap[index] = p.1
        }
        returned
    }

    pub fn update_from(&mut self, source: &SparseView<T>) {
        let screenspace_delta = (
            self.stencil.location.0.clone() - source.stencil.location.0.clone()
            , IntExp::ZERO - (self.stencil.location.1.clone() - source.stencil.location.1.clone())
            , self.stencil.location.2 - source.stencil.location.2
        );

        let source_is_preferred = source.stencil.urgency > self.stencil.urgency;

        match screenspace_delta.2.cmp(&0) {
            Ordering::Equal => {
                let pan_pixel_delta: (isize, isize) = (
                    screenspace_delta.0.shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT)
                        .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                    , (screenspace_delta.1).shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT)
                        .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                );


                for (_, _, (seat, row)) in &source.points {


                }
            }
            ,
            Ordering::Greater => {
                if screenspace_delta.2 < 16 {

                } else {
                    panic!("Unimplemented block!")
                }
            }
            ,
            Ordering::Less => {
                if -screenspace_delta.2 < 16 {

                } else {
                    panic!("Unimplemented block!")
                }
            }
        }
    }


}
