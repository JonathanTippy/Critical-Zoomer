// See comment at end

use std::cmp::Ordering;
use std::time::Instant;
use rug::Integer;
use crate::assemblies::structs::*;
use crate::constants::*;
use crate::utils::*; use crate::intexp::*;

fn line_segments_overlap(a: (IntExp, IntExp), b: (IntExp, IntExp)) -> bool {
    // left edge inclusive right edge limit
    (a.0 >= b.0 && a.0 < b.1)
        || (a.1 > b.0 && a.1 < b.1)
}

fn line_segment_a_is_subset_of_b(a: (IntExp, IntExp), b: (IntExp, IntExp)) -> bool {
    // left edge inclusive right edge limit
    (a.0 >= b.0 && a.0 < b.1)
        && (a.1 > b.0 && a.1 < b.1)
}

impl PointStencil {

    pub fn correct_precision(self) -> Self {
        PointStencil {
            location:(self.location.0.clone().set_precision(PIXELS_PER_UNIT_POT+self.location.2)
            , self.location.1.clone().set_precision(PIXELS_PER_UNIT_POT +self.location.2), self.location.2)
            , resolution: self.resolution
            , serial_number: self.serial_number
        }
    }
    pub fn assert_validity(&self) {
        assert!(
            self.location.0.exp == -(self.location.2 + PIXELS_PER_UNIT_POT)
            && self.location.0.exp == self.location.1.exp
            , "Invalid Stencil: POT zoom level and precision exponents must match."
        );
        assert!(
            self.resolution.0 < 2 << 16 && self.resolution.1 < 2 << 16
            , "Invalid Stencil: No resolution side length may exceed 2^16 pixels."
        );
        assert!(
            self.resolution.0 > 0 && self.resolution.1 > 0
            , "Invalid Stencil: No resolution side length may be 0 pixels."
        );
    }
    pub fn index(&self, seat_and_row: (isize, isize)) -> usize {
        debug_assert!(
            seat_and_row.0 >= 0 && seat_and_row.0 < self.resolution.0 as isize
                && seat_and_row.1 >= 0 && seat_and_row.1 < self.resolution.1 as isize
            , "Index Failure: nonexistent seat."
        );
        seat_and_row.1 as usize * self.resolution.0 + seat_and_row.0 as usize
    }
    pub fn seat_and_row(&self, index: usize) -> (usize, usize) {
        debug_assert!(
            index < self.resolution.0 * self.resolution.1
            , "Index Failure: nonexistent seat."
        );
        (index % self.resolution.0, index / self.resolution.0)
    }

    pub fn clamp_seat_and_row(&self, seat_and_row: (isize, isize)) -> (isize, isize) {
        return (
            seat_and_row.0.clamp(0, self.resolution.0 as isize - 1)
            , seat_and_row.1.clamp(0, self.resolution.1 as isize - 1)
        );
    }

    pub fn bottom_right_point(&self) -> (IntExp, IntExp) {
        let space = IntExp::from(1).shift(-self.location.2 - PIXELS_PER_UNIT_POT);
        return (
            self.location.0.clone() + space.clone() * IntExp::from(self.resolution.0-1)
            , self.location.1.clone() - space * IntExp::from(self.resolution.1-1)
        )
    }
    fn space(&self) -> IntExp {
        let one = IntExp::from(1);
        one.shift(-(self.location.2 + PIXELS_PER_UNIT_POT))
    }

    fn corners(&self) -> ((IntExp, IntExp), (IntExp, IntExp)) {
        let top_left: (IntExp, IntExp) = (self.location.0.clone(), self.location.1.clone());

        let bottom_right: (IntExp, IntExp) = (
            self.location.0.clone() + self.space() * IntExp::from(self.resolution.0)
            , self.location.1.clone() - self.space() * IntExp::from(self.resolution.1)
        );
        (
            top_left
            , bottom_right
        )
    }
    fn overlaps(&self, other: &Self) -> bool {
        line_segments_overlap(
            (self.corners().0.0, self.corners().1.0)
            , (other.corners().0.0, other.corners().1.0)
        ) && line_segments_overlap(
            (self.corners().0.1, self.corners().1.1)
            , (other.corners().0.1, other.corners().1.1)
        )
    }

    fn subset_of(&self, other: &Self) -> bool {
        line_segment_a_is_subset_of_b(
            (self.corners().0.0, self.corners().1.0)
            , (other.corners().0.0, other.corners().1.0)
        ) && line_segment_a_is_subset_of_b(
            (self.corners().0.1, self.corners().1.1)
            , (other.corners().0.1, other.corners().1.1)
        )
    }
}


impl<T: Copy + Clone> View<T> {
    pub fn new(stencil: PointStencil, fill_value: T) -> View<T> {
        let returned = View {
            stencil: stencil.clone().correct_precision()
            ,
            data: vec!(fill_value; stencil.resolution.0 * stencil.resolution.1)
            ,
            bitmap: vec!(0u8; stencil.resolution.0 * stencil.resolution.1)

        };
        returned.assert_validity();
        returned
    }

    pub fn new_known(stencil: PointStencil, fill_value: T) -> View<T> {
        let returned = View {
            stencil: stencil.clone().correct_precision()
            ,
            data: vec!(fill_value; stencil.resolution.0 * stencil.resolution.1)
            ,
            bitmap: vec!(EXACT+PROX; stencil.resolution.0 * stencil.resolution.1)
        };
        returned.assert_validity();
        returned
    }

    fn fill_rectangle(
        &mut self
        , top_left_seat: (isize, isize)
        , bottom_right_seat: (isize, isize)
        , top_left_is_exact: bool
        , fill_value: T
        , est: bool
        , source_is_preferred: bool
    ) {
        for row in top_left_seat.1..bottom_right_seat.1 {
            for seat in top_left_seat.0..bottom_right_seat.0 {
                let self_index = self.stencil.index((seat, row));
                if (seat, row) != top_left_seat || !top_left_is_exact {
                    let source_real_alignment = { if est { PROX } else { 0 }};

                    let self_alignment = self.bitmap[self.stencil.index((seat, row))];

                    if source_real_alignment >= self_alignment
                        || source_is_preferred && source_real_alignment >= self_alignment
                        || self_alignment == 0
                    {
                        self.data[self_index] = fill_value;
                        self.bitmap[self_index] = source_real_alignment;
                    }

                } else {
                    self.data[self_index] = fill_value;
                    self.bitmap[self_index] = EXACT + PROX;
                }
            }
        }
    }
}


impl<T: Copy> View<T> {
    pub fn assert_validity(&self) {
        self.stencil.assert_validity();
        assert_eq!(
            self.data.len(), self.stencil.resolution.0 * self.stencil.resolution.1
            , "Invalid View: Data length must equal seats times rows."
        );
        assert_eq!(
            self.data.len(),  self.bitmap.len()
            , "Invalid View: Data length must equal bitmap length."
        )
    }

    pub fn fill_from(&mut self, source: &Self) {

        self.assert_validity();

        let screenspace_delta = (
            self.stencil.location.0.clone() - source.stencil.location.0.clone()
            , IntExp::ZERO - (self.stencil.location.1.clone() - source.stencil.location.1.clone())
            , self.stencil.location.2 - source.stencil.location.2
        );

        let source_is_preferred = source.stencil.serial_number > self.stencil.serial_number;

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
                        *row = (row.saturating_add(pan_pixel_delta.1)).clamp((0 as isize), (source.stencil.resolution.1 as isize - 1));
                    };
                    clamped
                };
                let clamped_seats = {
                    let mut clamped: Vec<isize> = (0 as isize..self.stencil.resolution.0 as isize).collect();
                    for seat in &mut clamped {
                        *seat = (seat.saturating_add(pan_pixel_delta.0)).clamp((0 as isize), (source.stencil.resolution.0 as isize - 1));
                    };
                    clamped
                };

                for row in 0..self.stencil.resolution.1 {
                    for seat in 0..self.stencil.resolution.0 {
                        let preferred_source_seat_row = (
                            (seat as isize).saturating_add(pan_pixel_delta.0)
                            , (row as isize).saturating_add(pan_pixel_delta.1)
                        );

                        /*let clamped_source_seat_row = source
                            .stencil
                            .clamp_seat_and_row(preferred_source_seat_row);*/
                        let clamped_source_seat_row = (
                            clamped_seats[seat]
                            , clamped_rows[row]
                            );

                        let source_index = source.stencil.index(clamped_source_seat_row);
                        let self_index = self.stencil.index((seat as isize, row as isize));

                        let represented = preferred_source_seat_row == clamped_source_seat_row;
                        let value = source.data[source_index];
                        let source_alignment = source.bitmap[source_index];
                        let est = represented && source_alignment & PROX == PROX;
                        let exact = represented && source_alignment & EXACT == EXACT;

                        let source_real_alignment = { if exact { EXACT } else { 0 } } + { if est { PROX } else { 0 } };
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
                        pan_self_pixel_delta.0.saturating_sub(pan_source_pixel_delta.0 << screenspace_delta.2)
                        , pan_self_pixel_delta.1.saturating_sub(pan_source_pixel_delta.1 << screenspace_delta.2)
                    );

                    let frequency = 1 << screenspace_delta.2;

                    for row in 0..self.stencil.resolution.1 {
                        for seat in 0..self.stencil.resolution.0 {
                            // smaller pixels inherit top left larger pixel
                            let preferred_source_seat_row = (
                                ((seat as isize).saturating_add(pan_self_pixel_delta.0)) >> screenspace_delta.2
                                , ((row as isize).saturating_add(pan_self_pixel_delta.1)) >> screenspace_delta.2
                            );
                            // smaller pixels inherit closest larger pixel, bias top left on ties.
                            /*let preferred_source_seat_row = (
                                (seat as isize + pan_self_pixel_delta.0 + (frequency >> 1) - 1) >> screenspace_delta.2
                                , (row as isize + pan_self_pixel_delta.1 + (frequency >> 1) - 1) >> screenspace_delta.2
                            );*/

                            let aligned = ((seat as isize).saturating_sub(phase.0)) % frequency == 0
                                && ((row as isize).saturating_sub(phase.1)) % frequency == 0;

                            let clamped_source_seat_row = source
                                .stencil
                                .clamp_seat_and_row(preferred_source_seat_row);

                            let represented = preferred_source_seat_row == clamped_source_seat_row;
                            let value = source.data[source.stencil.index(clamped_source_seat_row)];
                            let source_old_alignment = source.bitmap[source.stencil.index(clamped_source_seat_row)];
                            let est = source_old_alignment & PROX == PROX && represented && aligned;
                            let exact = aligned && represented && source_old_alignment & EXACT == EXACT;

                            let source_alignment = { if exact { EXACT } else { 0 } } + { if est { PROX } else { 0 } };
                            let self_alignment = self.bitmap[self.stencil.index((seat as isize, row as isize))];

                            if source_alignment > self_alignment
                                || source_is_preferred && source_alignment >= self_alignment
                                || self_alignment == 0
                            {
                                self.data[self.stencil.index((seat as isize, row as isize))] = value;
                                self.bitmap[self.stencil.index((seat as isize, row as isize))] = source_alignment;
                            }
                        }
                    }
                } else {
                    // zooming in such that one pixel necessarily fills the entire screen.
                    // There are four cases, identified by whether the screen is split
                    // horizontally or vertically or both along pixel boundaries.

                    let self_bottom_right_corner = self.stencil.bottom_right_point();

                    let cut = (
                        self_bottom_right_corner.0
                            .set_precision(source.stencil.location.2+PIXELS_PER_UNIT_POT)
                        , self_bottom_right_corner.1
                            .set_precision(source.stencil.location.2 + PIXELS_PER_UNIT_POT)
                    );

                    let case:(Option<isize>, Option<isize>) = (
                        if cut.0 <= self.stencil.location.0 {
                            None
                        } else {
                            Some(
                                (cut.0.clone()-self.stencil.location.0.clone()).shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT).into()
                            )
                        }
                        , if cut.1 <= self.stencil.location.1 {
                            None
                        } else {
                            Some(
                                (cut.1.clone() - self.stencil.location.1.clone()).shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT).into()
                            )
                        }
                    );

                    match case {
                        (None, None) => {
                            let preferred_source_seat = (
                                <IntExp as Into<i32>>::into(cut.0.shift(source.stencil.location.2)).saturating_sub(1) as isize
                                , <IntExp as Into<i32>>::into(cut.1.shift(source.stencil.location.2)).saturating_sub(1) as isize
                            );
                            let clamped_source_seat = source.stencil.clamp_seat_and_row(preferred_source_seat);
                            let source_index= source.stencil.index(clamped_source_seat);
                            self.fill_rectangle(
                                (0, 0)
                                , (self.stencil.resolution.0 as isize, self.stencil.resolution.1 as isize)
                                , false
                                , source.data[source_index]
                                , source.bitmap[source_index] & PROX == PROX
                                    && clamped_source_seat==preferred_source_seat
                                , source.stencil.serial_number > self.stencil.serial_number
                            );
                        }
                        , (None, Some(vertical_edge)) => {

                            let preferred_source_seat_bottom = (
                                cut.0.shift(source.stencil.location.2).into()
                                , cut.1.shift(source.stencil.location.2).into()
                            );
                            let preferred_source_seat_top = (
                                preferred_source_seat_bottom.0
                                , preferred_source_seat_bottom.1 - 1
                            );
                            let clamped_source_seat_top = source.stencil.clamp_seat_and_row(preferred_source_seat_top);
                            let clamped_source_seat_bottom = source.stencil.clamp_seat_and_row(preferred_source_seat_bottom);

                            let source_index_top = source.stencil.index(clamped_source_seat_top);
                            let source_index_bottom = source.stencil.index(clamped_source_seat_bottom);
                            self.fill_rectangle(
                                (0, 0)
                                , (self.stencil.resolution.0 as isize, vertical_edge)
                                , false
                                , source.data[source_index_top]
                                , source.bitmap[source_index_top] & PROX == PROX
                                    && clamped_source_seat_top == preferred_source_seat_top
                                , source.stencil.serial_number > self.stencil.serial_number
                            );
                            self.fill_rectangle(
                                (0, vertical_edge)
                                , (self.stencil.resolution.0 as isize, self.stencil.resolution.1 as isize)
                                , false
                                , source.data[source_index_bottom]
                                , source.bitmap[source_index_bottom] & PROX == PROX
                                    && clamped_source_seat_bottom == preferred_source_seat_bottom
                                , source.stencil.serial_number > self.stencil.serial_number
                            );
                        }
                        , (Some(horizontal_edge), None) => {
                            let preferred_source_seat_right = (
                                cut.0.shift(source.stencil.location.2).into()
                                , cut.1.shift(source.stencil.location.2).into()
                            );
                            let preferred_source_seat_left = (
                                preferred_source_seat_right.0 - 1
                                , preferred_source_seat_right.1
                            );
                            let clamped_source_seat_right = source.stencil.clamp_seat_and_row(preferred_source_seat_right);
                            let clamped_source_seat_left = source.stencil.clamp_seat_and_row(preferred_source_seat_left);

                            let source_index_right = source.stencil.index(clamped_source_seat_right);
                            let source_index_left = source.stencil.index(clamped_source_seat_left);
                            self.fill_rectangle(
                                (0, 0)
                                , (horizontal_edge, self.stencil.resolution.1 as isize)
                                , false
                                , source.data[source_index_right]
                                , source.bitmap[source_index_right] & PROX == PROX
                                    && clamped_source_seat_right == preferred_source_seat_right
                                , source.stencil.serial_number > self.stencil.serial_number
                            );
                            self.fill_rectangle(
                                (horizontal_edge, 0)
                                , (self.stencil.resolution.0 as isize, self.stencil.resolution.1 as isize)
                                , false
                                , source.data[source_index_left]
                                , source.bitmap[source_index_left] & PROX == PROX
                                    && clamped_source_seat_left == preferred_source_seat_left
                                , source.stencil.serial_number > self.stencil.serial_number
                            );
                        }
                        ,
                        (Some(horizontal_edge), Some(vertical_edge)) => {
                            let preferred_source_seat_bottom_right = (
                                cut.0.shift(source.stencil.location.2).into()
                                , cut.1.shift(source.stencil.location.2).into()
                            );
                            let preferred_source_seat_bottom_left = (
                                preferred_source_seat_bottom_right.0 - 1
                                , preferred_source_seat_bottom_right.1
                            );
                            let preferred_source_seat_top_right = (
                                preferred_source_seat_bottom_right.0
                                , preferred_source_seat_bottom_right.1 - 1
                            );
                            let preferred_source_seat_top_left = (
                                preferred_source_seat_bottom_right.0 - 1
                                , preferred_source_seat_bottom_right.1 - 1
                            );

                            let clamped_source_seat_top_left = source.stencil.clamp_seat_and_row(preferred_source_seat_top_left);
                            let clamped_source_seat_top_right = source.stencil.clamp_seat_and_row(preferred_source_seat_top_right);
                            let clamped_source_seat_bottom_left = source.stencil.clamp_seat_and_row(preferred_source_seat_bottom_left);
                            let clamped_source_seat_bottom_right = source.stencil.clamp_seat_and_row(preferred_source_seat_bottom_right);

                            let source_index_top_left = source.stencil.index(clamped_source_seat_top_left);
                            let source_index_top_right = source.stencil.index(clamped_source_seat_top_right);
                            let source_index_bottom_left = source.stencil.index(clamped_source_seat_bottom_left);
                            let source_index_bottom_right = source.stencil.index(clamped_source_seat_bottom_right);

                            self.fill_rectangle(
                                (0, 0)
                                , (horizontal_edge, vertical_edge)
                                , false
                                , source.data[source_index_top_right]
                                , source.bitmap[source_index_top_right] & PROX == PROX
                                    && clamped_source_seat_top_right == preferred_source_seat_top_right
                                , source.stencil.serial_number > self.stencil.serial_number
                            );
                            self.fill_rectangle(
                                (horizontal_edge, 0)
                                , (self.stencil.resolution.0 as isize, vertical_edge)
                                , false
                                , source.data[source_index_top_left]
                                , source.bitmap[source_index_top_left] & PROX == PROX
                                    && clamped_source_seat_top_left == preferred_source_seat_top_left
                                , source.stencil.serial_number > self.stencil.serial_number
                            );
                            self.fill_rectangle(
                                (0, vertical_edge)
                                , (horizontal_edge, self.stencil.resolution.1 as isize)
                                , true
                                , source.data[source_index_bottom_right]
                                , source.bitmap[source_index_bottom_right] & PROX == PROX
                                    && clamped_source_seat_bottom_right == preferred_source_seat_bottom_right
                                , source.stencil.serial_number > self.stencil.serial_number
                            );
                            self.fill_rectangle(
                                (horizontal_edge, vertical_edge)
                                , (self.stencil.resolution.0 as isize, self.stencil.resolution.1 as isize)
                                , false
                                , source.data[source_index_bottom_left]
                                , source.bitmap[source_index_bottom_left] & PROX == PROX
                                    && clamped_source_seat_bottom_left == preferred_source_seat_bottom_left
                                , source.stencil.serial_number > self.stencil.serial_number
                            );
                        }
                    }
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

                    let frequency = 1 << -screenspace_delta.2;

                    for row in 0..self.stencil.resolution.1 {
                        for seat in 0..self.stencil.resolution.0 {
                            let preferred_source_seat_row = (
                                ((seat as isize).saturating_add(pan_self_pixel_delta.0)) << -screenspace_delta.2
                                , ((row as isize).saturating_add(pan_self_pixel_delta.1)) << -screenspace_delta.2
                            );


                            let clamped_source_seat_row = source
                                .stencil
                                .clamp_seat_and_row(preferred_source_seat_row);

                            let represented = preferred_source_seat_row == clamped_source_seat_row
                                && (clamped_source_seat_row.0 >> -screenspace_delta.2).saturating_sub(pan_self_pixel_delta.0) == seat as isize
                                && (clamped_source_seat_row.1 >> -screenspace_delta.2).saturating_sub(pan_self_pixel_delta.1) == row as isize;
                            let value = source.data[source.stencil.index(clamped_source_seat_row)];
                            let source_alignment = source.bitmap[source.stencil.index(clamped_source_seat_row)];
                            let exact = represented && source_alignment & EXACT == EXACT;
                            let est = represented && source_alignment & PROX == PROX;

                            let source_real_alignment = { if exact { EXACT } else { 0 } } + { if est { PROX } else { 0 } };
                            let self_alignment = self.bitmap[self.stencil.index((seat as isize, row as isize))];

                            if source_real_alignment > self_alignment
                                || source_is_preferred && source_real_alignment >= self_alignment
                                || self_alignment == 0
                            {
                                self.data[self.stencil.index((seat as isize, row as isize))] = value;
                                self.bitmap[self.stencil.index((seat as isize, row as isize))] = source_real_alignment;
                            }
                        }
                    }
                } else {
                    //large zoom out, very little relevant data
                    //TODO! decide what to do here
                }
            }
        }
    }
}

#[test]
#[should_panic]
fn invalid_test_bad_data() {
    let mut a: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!()
        ,
        bitmap: vec!()
        ,
    };
    let b: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!()
        ,
        bitmap: vec!()

        ,
    };
    a.fill_from(&b);
}

#[test]
#[should_panic]
fn invalid_test_misaligned() {
    let mut a: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp::ZERO
                , IntExp::ZERO
                , 0
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!()
        ,
        bitmap: vec!()

        ,
        
    };
    let b: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp::ZERO
                , IntExp::ZERO
                , 0
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!()
        ,
        bitmap: vec!()
        ,
        
    };
    a.fill_from(&b);
}

#[test]
fn identity_test() {
    let mut a: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(1, 2, 3, 4)
        ,
        bitmap: vec!(EXACT + PROX, EXACT + PROX, EXACT + PROX, EXACT + PROX)

        ,
        
    };
    let b: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(1, 2, 3, 4),
        bitmap: vec!(EXACT + PROX, EXACT + PROX, EXACT + PROX, EXACT + PROX),

    };
    a.fill_from(&b);
    if a.data != b.data {
        eprintln!("actual: {:?}", a.data);
        eprintln!("expect: {:?}", b.data);
    }
    assert!(a.data == b.data);
}

#[test]
fn improve_test() {
    let mut a: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(0, 0, 0, 0)
        ,
        bitmap: vec!(PROX, PROX, PROX, PROX),
        
    };
    let b: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(1, 2, 3, 4),
        bitmap: vec!(EXACT + PROX, EXACT + PROX, EXACT + PROX, EXACT + PROX),
        
    };
    a.fill_from(&b);
    if a.data != b.data {
        eprintln!("actual: {:?}", a.data);
        eprintln!("expect: {:?}", b.data);
    }
    assert!(a.data == b.data);
}

#[test]
fn zoom_in_test() {
    let mut a: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT - 1 }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT - 1 }
                , 1
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(0, 0, 0, 0)
        ,
        bitmap: vec!(0, 0, 0, 0),
        
    };
    let b: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(1, 2, 3, 4),
        bitmap: vec!(EXACT + PROX, EXACT + PROX, EXACT + PROX, EXACT + PROX),
        
    };

    let expect: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT - 1 }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT - 1 }
                , 1
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(1, 1, 1, 1),
        bitmap: vec!(EXACT + PROX, PROX, PROX, PROX),
        
    };
    a.fill_from(&b);
    if a.data != expect.data {
        eprintln!("actual: {:?}", a.data);
        eprintln!("expect: {:?}", expect.data);
    }
    assert!(a.data == expect.data);
}

#[test]
fn zoom_in_test_3() {
    let mut a: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT - 1 }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT - 1 }
                , 1
            )
            ,
            resolution: (3, 3),
            serial_number: 0
        }
        ,
        data: vec!(0, 0, 0, 0, 0, 0, 0, 0, 0)
        ,
        bitmap: vec!(0, 0, 0, 0, 0, 0, 0, 0, 0),

    };
    let b: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (3, 3),
            serial_number: 0
        }
        ,
        data: vec!(1, 2, 3, 4, 5, 6, 7, 8, 9),
        bitmap: vec!(EXACT + PROX, EXACT + PROX, EXACT + PROX, EXACT + PROX, EXACT + PROX, EXACT + PROX, EXACT + PROX, EXACT + PROX, EXACT + PROX,),

    };

    let expect: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT - 1 }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT - 1 }
                , 1
            )
            ,
            resolution: (2, 2),
            serial_number: 0
        }
        ,
        data: vec!(1, 1, 2, 1, 1, 2, 4, 4, 5),
        bitmap: vec!(EXACT + PROX, PROX, EXACT + PROX, PROX, PROX, PROX, PROX, EXACT + PROX, PROX, EXACT + PROX),

    };
    a.fill_from(&b);
    if a.data != expect.data {
        eprintln!("actual: {:?}", a.data);
        eprintln!("expect: {:?}", expect.data);
    }
    assert!(a.data == expect.data);
}


#[test]
fn zoom_out_test() {
    let mut a: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT + 1 }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT + 1 }
                , -1
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(0, 0, 0, 0)
        ,
        bitmap: vec!(0, 0, 0, 0),
        
    };
    let b: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(1, 2, 3, 4),
        bitmap: vec!(EXACT + PROX, EXACT + PROX, EXACT + PROX, EXACT + PROX),
        
    };

    let expect: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT + 1 }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT + 1 }
                , -1
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(1, 2, 3, 4),
        bitmap: vec!(EXACT + PROX, 0, 0, 0),
        
    };
    a.fill_from(&b);
    if a.data != expect.data {
        eprintln!("actual: {:?}", a.data);
        eprintln!("expect: {:?}", expect.data);
    }
    assert!(a.data == expect.data);
}

#[test]
fn pan_one_test() {
    let mut a: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(0, 0, 0, 0)
        ,
        bitmap: vec!(0, 0, 0, 0),
        
    };
    let b: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(1, 2, 3, 4),
        bitmap: vec!(EXACT + PROX, EXACT + PROX, EXACT + PROX, EXACT + PROX),
        
    };

    let expect: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(2, 2, 2, 2)
        ,
        bitmap: vec!(0, 0, EXACT + PROX, 0),
        
    };
    a.fill_from(&b);
    if a.data != expect.data {
        eprintln!("actual: {:?}", a.data);
        eprintln!("expect: {:?}", expect.data);
    }
    assert!(a.data == expect.data);
}

#[test]
fn nonzero_phase_test() {
    let mut a: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT - 1 }
                , IntExp { val: -Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT - 1 }
                , 1
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(0, 0, 0, 0)
        ,
        bitmap: vec!(0, 0, 0, 0),
        
    };
    let b: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(1, 2, 3, 4),
        bitmap: vec!(EXACT + PROX, EXACT + PROX, EXACT + PROX, EXACT + PROX),
        
    };

    let expect: View<i32> = View {
        stencil: PointStencil {
            location: (
                IntExp { val: Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT - 1 }
                , IntExp { val: -Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT - 1 }
                , 1
            )
            ,
            resolution: (2, 2), serial_number: 0
        }
        ,
        data: vec!(1, 2, 3, 4),
        bitmap: vec!(PROX, PROX, PROX, EXACT + PROX),
        
    };
    a.fill_from(&b);
    if a.data != expect.data {
        eprintln!("actual: {:?}", a.data);
        eprintln!("expect: {:?}", expect.data);
    }
    assert!(a.data == expect.data);
}





use proptest::prelude::*;
proptest!{
//    #![proptest_config(ProptestConfig::with_cases(2048))]
    #[test]
    fn zoom_in_associativity_test(
        location in prop_oneof![
            8 => (-32i128..32i128, -32i128..32i128),
            1 => (-1i128..1i128, -1i128..1i128),
            //1 => (i128::MIN..i128::MAX, i128::MIN..i128::MAX)
        ]
        , resolution in prop_oneof![
            1 => (1usize..100usize, 1usize..100usize),
            9 => (1usize..5usize, 1..5usize),
        ]
        , initial_zoom in prop_oneof![
            8 => -32i32..32i32,
            1 => Just(0i32),
            1 => Just(-16i32),
            1 => Just(16i32),
            1 => Just(-15i32),
            1 => Just(15i32),
            1 => Just(-8i32),
            1 => Just(8i32),
            1 => -1000000i32..1000000i32
        ]
        , zoom_delta_A in prop_oneof![
            8 => 0i32..32i32,
            1 => Just(0i32),
            1 => Just(16i32),
            1 => Just(15i32),
            1 => Just(8i32),
            1 => 0i32..1000000i32
        ]
        , zoom_delta_B in prop_oneof![
            8 => 0i32..32i32,
            1 => Just(0i32),
            1 => Just(16i32),
            1 => Just(15i32),
            1 => Just(8i32),
            1 => 0i32..1000000i32
        ]
    ) {


        let location = (
                IntExp { val: Integer::from(location.0), exp: -PIXELS_PER_UNIT_POT-initial_zoom }
                , IntExp { val: Integer::from(location.1), exp: -PIXELS_PER_UNIT_POT-initial_zoom }
                , initial_zoom
            );

        let stencil_A = PointStencil{
            resolution
            , location: location.clone()
            , serial_number: 0
        };

        let stencil_B = PointStencil{
            resolution
            , location: (
                location.0.clone().set_precision(PIXELS_PER_UNIT_POT+initial_zoom+zoom_delta_A)
                , location.1.clone().set_precision(PIXELS_PER_UNIT_POT+initial_zoom+zoom_delta_A)
                , initial_zoom + zoom_delta_A
            )
            , serial_number: 1
        };

        let stencil_C = PointStencil{
            resolution
            , location: (
                location.0.set_precision(PIXELS_PER_UNIT_POT+initial_zoom+zoom_delta_A+zoom_delta_B)
                , location.1.set_precision(PIXELS_PER_UNIT_POT+initial_zoom+zoom_delta_A+zoom_delta_B)
                , initial_zoom + zoom_delta_A + zoom_delta_B
            )
            , serial_number: 2
        };

        let mut source_view = View::new(stencil_A, 0);

        for seat in 0..resolution.0*resolution.1 {
            source_view.data[seat]=seat;
        }

        let mut one_step = View::new(stencil_C.clone(), 0);
        one_step.fill_from(&source_view);

        let mut two_step_one = View::new(stencil_B, 0);
        let mut two_step_two = View::new(stencil_C, 0);

        two_step_one.fill_from(&source_view);
        two_step_two.fill_from(&two_step_one);

        prop_assert_eq!(one_step, two_step_two);
    }
}



// Conventions:
// location.2 is magnification which is not the precision exponent.
// magnification goes up as you zoom in.
// Usually, when seat and row go in a tuple together, the order is seat then row.
// This is to align better with the x then y standard order.
// W/H and Width / Height are banned. This project uses seats and rows,
// and anytime both dimensions are together, a tuple called resolution.

// The stencil defines the set of complex points that make up a view.
// It is used with a vec equal to resolution.0 * resolution.1 in length.
// The top left sample of the view is taken exactly at location.0, location.1
// Other samples apply a regular grid,
// following imaginary coordinates: down is negative and right is positive.
// In complex plane terms: +seat moves +real; +row moves −imag.
// Scanning map between vec and pixels is done right then down, like a CRT.
// The points are equally spaced vertically and horizontally.
// The default points per unit is defined by the PIXELS_PER_UNIT_POT constant.
// The zoom level (location.2) is added to the constant to get the current PPU POT.
// The actual spacing distance between points is given by 1/(2^(PPU POT)).

// When filling one View from another, pixels are considered to represent:
// the area from their top left corner (inclusive) to their bottom right corner (limit).
// inexact mappings of larger to smaller are thusly fully defined.
// Optionally, the inexact (less important) values can be determined with a half-offset to
// closer approximate the nearest value and mitigate visual layout shift.

// Importantly, exact values are maintained and checked so that there are always some exact plotted pixels.
// This way, the results are "pixel imperfect": 2x zoom looks the same as a shift right,
// but greater zooms follow the rule that exact pixels don't represent an area,
// but a perfect plotted point. inexact pixels are filled best-effort.
// The best known algorithm for this is nearest with top left bias.
// A .5px bias will be present for the whole frame, which is easily accounted for and not visually noticeable.
// EDIT: unproven; likely to introduce a small error.
// Shelfed this concept because fill_from must yield the same result for a 4x zoom and two 2x zooms for example;
// functions which combine views must be associative.

// The complex plane is effectively divided into squares
// , where every smaller & larger pair where larger contains smaller can map small (choose top left) -> large or large (top left) -> many small

// The method to find at least one exactly mapped pixel if one exists is to check:
// 1. overlap (do the frame areas touch at all?) -> overlapping corner(s)
// 2. compatibility
// (does the relative offset contain units smaller than the smaller space? if so, no exact matches.)
// EDIT: no longer true; precision is folded in. each frame has a precision mapping its magnificaoitn level,
// so if there is overlap, there is compatibility.

// mapping is exact when one mapped exact pixel is identified,
// and the larger pixel step off of that pixel yields pixels still represented in the smaller pixel view.
