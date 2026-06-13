use std::cmp::Ordering;
use crate::assemblies::structs::*;
use std::collections::*;
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::utils::IntExp;

pub struct SparseView<T> {
    stencil: PixelStencil
    , points: Vec<(T, u8, (usize, usize))>
    , map: HashMap<(isize, isize), usize>
}

impl<T: Copy + Clone> SparseView<T> {

    pub fn insert(&mut self, new: (T, (usize, usize))) {
        
    }

    pub fn into(self, fill_value: T) -> View<T> {
        let mut returned = View::new(self.stencil, fill_value);
        for p in self.points {
            returned.data[p.2] = p.0;
            returned.bitmap[p.2] = p.1
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

                let clamped_rows = {
                    let mut clamped: Vec<isize> = (0 as isize..self.stencil.resolution.1 as isize).collect();
                    for row in &mut clamped {
                        *row = (*row + pan_pixel_delta.1).clamp((0 as isize), (source.stencil.resolution.1 as isize - 1));
                    };
                    clamped
                };
                let clamped_seats = {
                    let mut clamped: Vec<isize> = (0 as isize..self.stencil.resolution.0 as isize).collect();
                    for seat in &mut clamped {
                        *seat = (*seat + pan_pixel_delta.0).clamp((0 as isize), (source.stencil.resolution.0 as isize - 1));
                    };
                    clamped
                };

                for (_, _, (seat, row)) in source.points {
                    let preferred_source_seat_row = (
                        seat as isize, row as isize
                    );

                    /*let clamped_source_seat_row = source
                        .stencil
                        .clamp_seat_and_row(preferred_source_seat_row);*/
                    let clamped_source_seat_row = (
                        clamped_seats[seat]
                        , clamped_rows[row]
                    );

                    let source_index = source.stencil.index_trust_input(clamped_source_seat_row);
                    let self_index = self.stencil.index_trust_input((seat as isize, row as isize));

                    let representative = preferred_source_seat_row == clamped_source_seat_row;
                    let value = source.data[source_index];
                    let source_alignment = source.bitmap[source_index];
                    let exact = representative && source_alignment & EXACT == EXACT;

                    let source_real_alignment = { if exact { EXACT } else { 0 } } + { if representative { EST } else { 0 } };
                    let self_alignment = self.bitmap[self_index];

                    if source_real_alignment >= self_alignment
                        || source_is_preferred && source_real_alignment >= self_alignment
                        || self_alignment == 0
                    {
                        self.data[self_index] = value;
                        self.bitmap[self_index] = source_real_alignment;
                    }

                }
            }
            ,
            Ordering::Greater => {
                if screenspace_delta.2 < 16 {
                    let pan_self_pixel_delta: (isize, isize) = (
                        screenspace_delta.0.clone().shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                        , (screenspace_delta.1.clone()).shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                    );

                    let pan_source_pixel_delta: (isize, isize) = (
                        screenspace_delta.0.clone().shift(source.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                        , (screenspace_delta.1.clone()).shift(source.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                    );

                    let phase = (
                        pan_self_pixel_delta.0 - (pan_source_pixel_delta.0 << screenspace_delta.2)
                        , pan_self_pixel_delta.1 - (pan_source_pixel_delta.1 << screenspace_delta.2)
                    );

                    let frequency = 1 << screenspace_delta.2;

                    for row in 0..self.stencil.resolution.1 {
                        for seat in 0..self.stencil.resolution.0 {
                            let preferred_source_seat_row = (
                                (seat as isize + pan_self_pixel_delta.0) >> screenspace_delta.2
                                , (row as isize + pan_self_pixel_delta.1) >> screenspace_delta.2
                            );

                            let aligned = (seat as isize - phase.0) % frequency == 0
                                && (row as isize - phase.1) % frequency == 0;

                            let clamped_source_seat_row = source
                                .stencil
                                .clamp_seat_and_row(preferred_source_seat_row);

                            let representative = preferred_source_seat_row == clamped_source_seat_row;
                            let value = source.data[source.stencil.index_trust_input(clamped_source_seat_row)];
                            let source_old_alignment = source.bitmap[source.stencil.index_trust_input(clamped_source_seat_row)];
                            let exact = representative && source_old_alignment & EXACT == EXACT;

                            let source_alignment = { if exact { EXACT } else { 0 } } + { if representative { EST } else { 0 } };
                            let self_alignment = self.bitmap[self.stencil.index_trust_input((seat as isize, row as isize))];

                            if source_alignment > self_alignment
                                || source_is_preferred && source_alignment >= self_alignment
                                || self_alignment == 0
                            {
                                self.data[self.stencil.index_trust_input((seat as isize, row as isize))] = value;
                                self.bitmap[self.stencil.index_trust_input((seat as isize, row as isize))] = source_alignment;
                            }
                        }
                    }
                } else {
                    panic!("Unimplemented block!")
                }
            }
            ,
            Ordering::Less => {
                if screenspace_delta.2 > -16 {
                    let pan_source_pixel_delta: (isize, isize) = (
                        screenspace_delta.0.clone().shift(source.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                        , (screenspace_delta.1.clone()).shift(source.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                    );

                    let pan_self_pixel_delta: (isize, isize) = (
                        screenspace_delta.0.clone().shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                        , (screenspace_delta.1.clone()).shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                    );

                    let phase = (
                        pan_source_pixel_delta.0 - (pan_self_pixel_delta.0 << screenspace_delta.2)
                        , pan_source_pixel_delta.1 - (pan_self_pixel_delta.1 << screenspace_delta.2)
                    );

                    let frequency = 1 << -screenspace_delta.2;

                    for row in 0..self.stencil.resolution.1 {
                        for seat in 0..self.stencil.resolution.0 {
                            let preferred_source_seat_row = (
                                (seat as isize + pan_self_pixel_delta.0) << -screenspace_delta.2
                                , (row as isize + pan_self_pixel_delta.1) << -screenspace_delta.2
                            );

                            let aligned = (preferred_source_seat_row.0 - phase.0) % frequency == 0
                                && (preferred_source_seat_row.1 - phase.1) % frequency == 0;

                            let clamped_source_seat_row = source
                                .stencil
                                .clamp_seat_and_row(preferred_source_seat_row);

                            let representative = preferred_source_seat_row == clamped_source_seat_row;
                            let value = source.data[source.stencil.index_trust_input(clamped_source_seat_row)];
                            let source_alignment = source.bitmap[source.stencil.index_trust_input(clamped_source_seat_row)];
                            let exact = representative && source_alignment & EXACT == EXACT;

                            let source_real_alignment = { if exact { EXACT } else { 0 } } + { if representative { EST } else { 0 } };
                            let self_alignment = self.bitmap[self.stencil.index_trust_input((seat as isize, row as isize))];

                            if source_real_alignment > self_alignment
                                || source_is_preferred && source_real_alignment >= self_alignment
                                || self_alignment == 0
                            {
                                self.data[self.stencil.index_trust_input((seat as isize, row as isize))] = value;
                                self.bitmap[self.stencil.index_trust_input((seat as isize, row as isize))] = source_real_alignment;
                            }
                        }
                    }
                } else {
                    panic!("Unimplemented block!")
                }
            }
        }
    }


}
