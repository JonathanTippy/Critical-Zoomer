use std::cmp::Ordering;
use std::collections::HashMap;
use crate::assemblies::structs::{PointStencil, View, EXACT, PROX};
use crate::assemblies::workgroup_new::structs::SparseView;
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::utils::IntExp;

impl<T: Copy + Clone> SparseView<T> {
    pub fn new(stencil: PointStencil) -> SparseView<T> {
        SparseView {
            stencil
            ,
            points: vec!()
            ,
            map: HashMap::new()
        }
    }

    pub fn insert(&mut self, new: (T, (usize, usize))) {
        match self.map.get(&new.1) {
            None => {
                let index = self.points.len();
                self.points.push((new.0, EXACT + PROX, new.1));
                self.map.insert(new.1, index);
            }
            Some(s) => {
                self.points[*s] = (new.0, EXACT + PROX, new.1);
            }
        }
    }
    pub fn insert_with_align(&mut self, new: (T, u8, (usize, usize))) {
        match self.map.get(&new.2) {
            None => {
                let index = self.points.len();
                self.points.push((new.0, new.1, new.2));
                self.map.insert(new.2, index);
            }
            Some(s) => {
                self.points[*s] = (new.0, new.1, new.2);
            }
        }
    }

    pub fn into_view(self, fill_value: T) -> View<T> {
        let mut returned = View::new(self.stencil, fill_value);
        for p in self.points {
            let index = returned.stencil.index((p.2.0 as isize, p.2.1 as isize));
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

        let source_is_preferred = source.stencil.serial_number > self.stencil.serial_number;

        match screenspace_delta.2.cmp(&0) {
            Ordering::Equal => {
                let pan_pixel_delta: (isize, isize) = (
                    screenspace_delta.0.shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT)
                        .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                    , screenspace_delta.1.shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT)
                        .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                );

                for &(value, source_alignment, (source_seat, source_row)) in &source.points {
                    let seat = (source_seat as isize).saturating_sub(pan_pixel_delta.0);
                    let row = (source_row as isize).saturating_sub(pan_pixel_delta.1);

                    if seat < 0
                        || row < 0
                        || seat >= self.stencil.resolution.0 as isize
                        || row >= self.stencil.resolution.1 as isize
                    { continue; }

                    let seat_and_row = (seat as usize, row as usize);
                    let exact = source_alignment & EXACT == EXACT;
                    let source_real_alignment =
                        (if exact { EXACT } else { 0 }) + PROX;

                    let self_alignment = self.map.get(&seat_and_row)
                        .map(|&index| self.points[index].1)
                        .unwrap_or(0);

                    if source_real_alignment >= self_alignment
                        || source_is_preferred && source_real_alignment >= self_alignment
                        || self_alignment == 0
                    {
                        match self.map.get(&seat_and_row) {
                            Some(&index) => {
                                self.points[index] = (value, source_real_alignment, seat_and_row);
                            }
                            None => {
                                let index = self.points.len();
                                self.points.push((value, source_real_alignment, seat_and_row));
                                self.map.insert(seat_and_row, index);
                            }
                        }
                    }
                }
            }
            Ordering::Greater => {
                if screenspace_delta.2 < 16 {
                    let pan_sink_pixel_delta: (isize, isize) = (
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
                        pan_sink_pixel_delta.0.saturating_sub(pan_source_pixel_delta.0 << screenspace_delta.2)
                        , pan_sink_pixel_delta.1.saturating_sub(pan_source_pixel_delta.1 << screenspace_delta.2)
                    );

                    let frequency = 1 << screenspace_delta.2;

                    for &(value, source_alignment, (source_seat, source_row)) in &source.points {
                        let sink_top_left_seat = (
                            ((source_seat as isize) << screenspace_delta.2).saturating_sub(pan_sink_pixel_delta.0)
                            , ((source_row as isize) << screenspace_delta.2).saturating_sub(pan_sink_pixel_delta.1)
                        );
                        let sink_bottom_right_seat = (
                            (((source_seat as isize) + 1) << screenspace_delta.2).saturating_sub(pan_sink_pixel_delta.0)
                            , (((source_row as isize) + 1) << screenspace_delta.2).saturating_sub(pan_sink_pixel_delta.1)
                        );

                        let mut sink_left_seat = sink_top_left_seat.0;
                        let sink_seat_remainder = (sink_left_seat.saturating_sub(phase.0)) % frequency;
                        if sink_seat_remainder != 0 { sink_left_seat = sink_left_seat.saturating_add(frequency - sink_seat_remainder); }

                        for seat in (sink_left_seat..sink_bottom_right_seat.0).step_by(frequency as usize) {
                            let mut sink_top_row = sink_top_left_seat.1;
                            let sink_row_remainder = (sink_top_row.saturating_sub(phase.1)) % frequency;
                            if sink_row_remainder != 0 { sink_top_row = sink_top_row.saturating_add(frequency - sink_row_remainder); }

                            for row in (sink_top_row..sink_bottom_right_seat.1).step_by(frequency as usize) {
                                if seat < 0 || row < 0
                                    || seat >= self.stencil.resolution.0 as isize
                                    || row >= self.stencil.resolution.1 as isize
                                { continue; }

                                let seat_and_row = (seat as usize, row as usize);
                                let exact = source_alignment & EXACT == EXACT;
                                let est = source_alignment & PROX == PROX;
                                let source_real_alignment =
                                    { if exact { EXACT } else { 0 } } + { if est { PROX } else { 0 } };

                                if source_real_alignment == 0 { continue; }

                                let self_alignment = self.map.get(&seat_and_row)
                                    .map(|&index| self.points[index].1)
                                    .unwrap_or(0);

                                if source_real_alignment > self_alignment
                                    || source_is_preferred && source_real_alignment >= self_alignment
                                    || self_alignment == 0
                                {
                                    match self.map.get(&seat_and_row) {
                                        Some(&index) => {
                                            self.points[index] = (value, source_real_alignment, seat_and_row);
                                        }
                                        None => {
                                            let index = self.points.len();
                                            self.points.push((value, source_real_alignment, seat_and_row));
                                            self.map.insert(seat_and_row, index);
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // zooming in such that one pixel necessarily fills the entire screen.
                    // There are four cases, identified by whether the screen is split
                    // horizontally or vertically or both along pixel boundaries.

                    let sink_bottom_right_corner = self.stencil.bottom_right_point();

                    let cut = (
                        sink_bottom_right_corner.0
                            .set_precision(source.stencil.location.2 + PIXELS_PER_UNIT_POT)
                        , sink_bottom_right_corner.1
                            .set_precision(source.stencil.location.2 + PIXELS_PER_UNIT_POT)
                    );

                    let case: (Option<isize>, Option<isize>) = (
                        if cut.0 <= self.stencil.location.0 {
                            None
                        } else {
                            Some(
                                (cut.0.clone() - self.stencil.location.0.clone()).shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT).into()
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
                            let sink_top_left_seat = (0, 0);
                            let sink_bottom_right_seat = (
                                self.stencil.resolution.0 as isize
                                , self.stencil.resolution.1 as isize
                            );
                            for &(value, source_alignment, (source_seat, source_row)) in &source.points {
                                if (source_seat as isize, source_row as isize) != preferred_source_seat { continue; }
                                let est = source_alignment & PROX == PROX;
                                for row in sink_top_left_seat.1..sink_bottom_right_seat.1 {
                                    for seat in sink_top_left_seat.0..sink_bottom_right_seat.0 {
                                        let seat_and_row = (seat as usize, row as usize);
                                        let source_real_alignment = { if est { PROX } else { 0 } };
                                        if source_real_alignment == 0 { continue; }
                                        let self_alignment = self.map.get(&seat_and_row)
                                            .map(|&index| self.points[index].1)
                                            .unwrap_or(0);
                                        if source_real_alignment >= self_alignment
                                            || source_is_preferred && source_real_alignment >= self_alignment
                                            || self_alignment == 0
                                        {
                                            match self.map.get(&seat_and_row) {
                                                Some(&index) => {
                                                    self.points[index] = (value, source_real_alignment, seat_and_row);
                                                }
                                                None => {
                                                    let index = self.points.len();
                                                    self.points.push((value, source_real_alignment, seat_and_row));
                                                    self.map.insert(seat_and_row, index);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        (None, Some(vertical_edge)) => {
                            let preferred_source_seat_bottom = (
                                cut.0.shift(source.stencil.location.2).into()
                                , cut.1.shift(source.stencil.location.2).into()
                            );
                            let preferred_source_seat_top = (
                                preferred_source_seat_bottom.0
                                , preferred_source_seat_bottom.1 - 1
                            );
                            for &(value, source_alignment, (source_seat, source_row)) in &source.points {
                                let source_seat_row = (source_seat as isize, source_row as isize);
                                let est = source_alignment & PROX == PROX;
                                if source_seat_row == preferred_source_seat_top {
                                    let sink_top_left_seat = (0, 0);
                                    let sink_bottom_right_seat = (
                                        self.stencil.resolution.0 as isize
                                        , vertical_edge
                                    );
                                    for row in sink_top_left_seat.1..sink_bottom_right_seat.1 {
                                        for seat in sink_top_left_seat.0..sink_bottom_right_seat.0 {
                                            let seat_and_row = (seat as usize, row as usize);
                                            let source_real_alignment = { if est { PROX } else { 0 } };
                                            if source_real_alignment == 0 { continue; }
                                            let self_alignment = self.map.get(&seat_and_row)
                                                .map(|&index| self.points[index].1)
                                                .unwrap_or(0);
                                            if source_real_alignment >= self_alignment
                                                || source_is_preferred && source_real_alignment >= self_alignment
                                                || self_alignment == 0
                                            {
                                                match self.map.get(&seat_and_row) {
                                                    Some(&index) => {
                                                        self.points[index] = (value, source_real_alignment, seat_and_row);
                                                    }
                                                    None => {
                                                        let index = self.points.len();
                                                        self.points.push((value, source_real_alignment, seat_and_row));
                                                        self.map.insert(seat_and_row, index);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else if source_seat_row == preferred_source_seat_bottom {
                                    let sink_top_left_seat = (0, vertical_edge);
                                    let sink_bottom_right_seat = (
                                        self.stencil.resolution.0 as isize
                                        , self.stencil.resolution.1 as isize
                                    );
                                    for row in sink_top_left_seat.1..sink_bottom_right_seat.1 {
                                        for seat in sink_top_left_seat.0..sink_bottom_right_seat.0 {
                                            let seat_and_row = (seat as usize, row as usize);
                                            let source_real_alignment = { if est { PROX } else { 0 } };
                                            if source_real_alignment == 0 { continue; }
                                            let self_alignment = self.map.get(&seat_and_row)
                                                .map(|&index| self.points[index].1)
                                                .unwrap_or(0);
                                            if source_real_alignment >= self_alignment
                                                || source_is_preferred && source_real_alignment >= self_alignment
                                                || self_alignment == 0
                                            {
                                                match self.map.get(&seat_and_row) {
                                                    Some(&index) => {
                                                        self.points[index] = (value, source_real_alignment, seat_and_row);
                                                    }
                                                    None => {
                                                        let index = self.points.len();
                                                        self.points.push((value, source_real_alignment, seat_and_row));
                                                        self.map.insert(seat_and_row, index);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        (Some(horizontal_edge), None) => {
                            let preferred_source_seat_right = (
                                cut.0.shift(source.stencil.location.2).into()
                                , cut.1.shift(source.stencil.location.2).into()
                            );
                            let preferred_source_seat_left = (
                                preferred_source_seat_right.0 - 1
                                , preferred_source_seat_right.1
                            );
                            for &(value, source_alignment, (source_seat, source_row)) in &source.points {
                                let source_seat_row = (source_seat as isize, source_row as isize);
                                let est = source_alignment & PROX == PROX;
                                if source_seat_row == preferred_source_seat_right {
                                    let sink_top_left_seat = (0, 0);
                                    let sink_bottom_right_seat = (
                                        horizontal_edge
                                        , self.stencil.resolution.1 as isize
                                    );
                                    for row in sink_top_left_seat.1..sink_bottom_right_seat.1 {
                                        for seat in sink_top_left_seat.0..sink_bottom_right_seat.0 {
                                            let seat_and_row = (seat as usize, row as usize);
                                            let source_real_alignment = { if est { PROX } else { 0 } };
                                            if source_real_alignment == 0 { continue; }
                                            let self_alignment = self.map.get(&seat_and_row)
                                                .map(|&index| self.points[index].1)
                                                .unwrap_or(0);
                                            if source_real_alignment >= self_alignment
                                                || source_is_preferred && source_real_alignment >= self_alignment
                                                || self_alignment == 0
                                            {
                                                match self.map.get(&seat_and_row) {
                                                    Some(&index) => {
                                                        self.points[index] = (value, source_real_alignment, seat_and_row);
                                                    }
                                                    None => {
                                                        let index = self.points.len();
                                                        self.points.push((value, source_real_alignment, seat_and_row));
                                                        self.map.insert(seat_and_row, index);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else if source_seat_row == preferred_source_seat_left {
                                    let sink_top_left_seat = (horizontal_edge, 0);
                                    let sink_bottom_right_seat = (
                                        self.stencil.resolution.0 as isize
                                        , self.stencil.resolution.1 as isize
                                    );
                                    for row in sink_top_left_seat.1..sink_bottom_right_seat.1 {
                                        for seat in sink_top_left_seat.0..sink_bottom_right_seat.0 {
                                            let seat_and_row = (seat as usize, row as usize);
                                            let source_real_alignment = { if est { PROX } else { 0 } };
                                            if source_real_alignment == 0 { continue; }
                                            let self_alignment = self.map.get(&seat_and_row)
                                                .map(|&index| self.points[index].1)
                                                .unwrap_or(0);
                                            if source_real_alignment >= self_alignment
                                                || source_is_preferred && source_real_alignment >= self_alignment
                                                || self_alignment == 0
                                            {
                                                match self.map.get(&seat_and_row) {
                                                    Some(&index) => {
                                                        self.points[index] = (value, source_real_alignment, seat_and_row);
                                                    }
                                                    None => {
                                                        let index = self.points.len();
                                                        self.points.push((value, source_real_alignment, seat_and_row));
                                                        self.map.insert(seat_and_row, index);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
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
                            for &(value, source_alignment, (source_seat, source_row)) in &source.points {
                                let source_seat_row = (source_seat as isize, source_row as isize);
                                let est = source_alignment & PROX == PROX;
                                if source_seat_row == preferred_source_seat_top_right {
                                    let sink_top_left_seat = (0, 0);
                                    let sink_bottom_right_seat = (horizontal_edge, vertical_edge);
                                    for row in sink_top_left_seat.1..sink_bottom_right_seat.1 {
                                        for seat in sink_top_left_seat.0..sink_bottom_right_seat.0 {
                                            let seat_and_row = (seat as usize, row as usize);
                                            let source_real_alignment = { if est { PROX } else { 0 } };
                                            if source_real_alignment == 0 { continue; }
                                            let self_alignment = self.map.get(&seat_and_row)
                                                .map(|&index| self.points[index].1)
                                                .unwrap_or(0);
                                            if source_real_alignment >= self_alignment
                                                || source_is_preferred && source_real_alignment >= self_alignment
                                                || self_alignment == 0
                                            {
                                                match self.map.get(&seat_and_row) {
                                                    Some(&index) => {
                                                        self.points[index] = (value, source_real_alignment, seat_and_row);
                                                    }
                                                    None => {
                                                        let index = self.points.len();
                                                        self.points.push((value, source_real_alignment, seat_and_row));
                                                        self.map.insert(seat_and_row, index);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else if source_seat_row == preferred_source_seat_top_left {
                                    let sink_top_left_seat = (horizontal_edge, 0);
                                    let sink_bottom_right_seat = (
                                        self.stencil.resolution.0 as isize
                                        , vertical_edge
                                    );
                                    for row in sink_top_left_seat.1..sink_bottom_right_seat.1 {
                                        for seat in sink_top_left_seat.0..sink_bottom_right_seat.0 {
                                            let seat_and_row = (seat as usize, row as usize);
                                            let source_real_alignment = { if est { PROX } else { 0 } };
                                            if source_real_alignment == 0 { continue; }
                                            let self_alignment = self.map.get(&seat_and_row)
                                                .map(|&index| self.points[index].1)
                                                .unwrap_or(0);
                                            if source_real_alignment >= self_alignment
                                                || source_is_preferred && source_real_alignment >= self_alignment
                                                || self_alignment == 0
                                            {
                                                match self.map.get(&seat_and_row) {
                                                    Some(&index) => {
                                                        self.points[index] = (value, source_real_alignment, seat_and_row);
                                                    }
                                                    None => {
                                                        let index = self.points.len();
                                                        self.points.push((value, source_real_alignment, seat_and_row));
                                                        self.map.insert(seat_and_row, index);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else if source_seat_row == preferred_source_seat_bottom_right {
                                    let sink_top_left_seat = (0, vertical_edge);
                                    let sink_bottom_right_seat = (
                                        horizontal_edge
                                        , self.stencil.resolution.1 as isize
                                    );
                                    for row in sink_top_left_seat.1..sink_bottom_right_seat.1 {
                                        for seat in sink_top_left_seat.0..sink_bottom_right_seat.0 {
                                            let seat_and_row = (seat as usize, row as usize);
                                            let source_real_alignment =
                                                if (seat, row) == sink_top_left_seat {
                                                    EXACT + PROX
                                                } else {
                                                    { if est { PROX } else { 0 } }
                                                };
                                            if source_real_alignment == 0 { continue; }
                                            let self_alignment = self.map.get(&seat_and_row)
                                                .map(|&index| self.points[index].1)
                                                .unwrap_or(0);
                                            if source_real_alignment >= self_alignment
                                                || source_is_preferred && source_real_alignment >= self_alignment
                                                || self_alignment == 0
                                            {
                                                match self.map.get(&seat_and_row) {
                                                    Some(&index) => {
                                                        self.points[index] = (value, source_real_alignment, seat_and_row);
                                                    }
                                                    None => {
                                                        let index = self.points.len();
                                                        self.points.push((value, source_real_alignment, seat_and_row));
                                                        self.map.insert(seat_and_row, index);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else if source_seat_row == preferred_source_seat_bottom_left {
                                    let sink_top_left_seat = (horizontal_edge, vertical_edge);
                                    let sink_bottom_right_seat = (
                                        self.stencil.resolution.0 as isize
                                        , self.stencil.resolution.1 as isize
                                    );
                                    for row in sink_top_left_seat.1..sink_bottom_right_seat.1 {
                                        for seat in sink_top_left_seat.0..sink_bottom_right_seat.0 {
                                            let seat_and_row = (seat as usize, row as usize);
                                            let source_real_alignment = { if est { PROX } else { 0 } };
                                            if source_real_alignment == 0 { continue; }
                                            let self_alignment = self.map.get(&seat_and_row)
                                                .map(|&index| self.points[index].1)
                                                .unwrap_or(0);
                                            if source_real_alignment >= self_alignment
                                                || source_is_preferred && source_real_alignment >= self_alignment
                                                || self_alignment == 0
                                            {
                                                match self.map.get(&seat_and_row) {
                                                    Some(&index) => {
                                                        self.points[index] = (value, source_real_alignment, seat_and_row);
                                                    }
                                                    None => {
                                                        let index = self.points.len();
                                                        self.points.push((value, source_real_alignment, seat_and_row));
                                                        self.map.insert(seat_and_row, index);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Ordering::Less => {
                if -screenspace_delta.2 < 16 {
                    let pan_sink_pixel_delta: (isize, isize) = (
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

                    let magnitude = -screenspace_delta.2;

                    for &(value, source_alignment, (source_seat, source_row)) in &source.points {
                        let sink_seat = ((source_seat as isize) >> magnitude).saturating_sub(pan_sink_pixel_delta.0);
                        let sink_row = ((source_row as isize) >> magnitude).saturating_sub(pan_sink_pixel_delta.1);

                        let preferred_source_seat =
                            (sink_seat.saturating_add(pan_sink_pixel_delta.0)) << magnitude;
                        let preferred_source_row =
                            (sink_row.saturating_add(pan_sink_pixel_delta.1)) << magnitude;

                        if preferred_source_seat != source_seat as isize
                            || preferred_source_row != source_row as isize
                        { continue; }

                        if sink_seat < 0 || sink_row < 0
                            || sink_seat >= self.stencil.resolution.0 as isize
                            || sink_row >= self.stencil.resolution.1 as isize
                        { continue; }

                        let seat_and_row = (sink_seat as usize, sink_row as usize);
                        let exact = source_alignment & EXACT == EXACT;
                        let est = source_alignment & PROX == PROX;
                        let source_real_alignment =
                            { if exact { EXACT } else { 0 } } + { if est { PROX } else { 0 } };

                        if source_real_alignment == 0 { continue; }

                        let self_alignment = self.map.get(&seat_and_row)
                            .map(|&index| self.points[index].1)
                            .unwrap_or(0);

                        if source_real_alignment > self_alignment
                            || source_is_preferred && source_real_alignment >= self_alignment
                            || self_alignment == 0
                        {
                            match self.map.get(&seat_and_row) {
                                Some(&index) => {
                                    self.points[index] = (value, source_real_alignment, seat_and_row);
                                }
                                None => {
                                    let index = self.points.len();
                                    self.points.push((value, source_real_alignment, seat_and_row));
                                    self.map.insert(seat_and_row, index);
                                }
                            }
                        }
                    }
                } else {
                    //panic!("Unimplemented block!")
                }
            }
        }
    }
}


use rug::Integer;

use proptest::prelude::*;
proptest! {
//    #![proptest_config(ProptestConfig::with_cases(2048))]
    #[test]
    fn test_sparse_view_parity(
        resolution in prop_oneof![
            1 => (1usize..100usize, 1usize..100usize),
            9 => (1usize..5usize, 1..5usize),
        ]
        , location in prop_oneof![
            8 => (-32i128..32i128, -32i128..32i128),
            1 => (-1i128..1i128, -1i128..1i128),
            //1 => (i128::MIN..i128::MAX, i128::MIN..i128::MAX)
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
        , location_delta in prop_oneof![
            8 => (-32i128..32i128, -32i128..32i128),
            1 => (-1i128..1i128, -1i128..1i128),
            //1 => (i128::MIN..i128::MAX, i128::MIN..i128::MAX)
        ]
        , zoom_delta in prop_oneof![
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
    ) {

        let location_delta = (
                IntExp { val: Integer::from(location_delta.0), exp: -PIXELS_PER_UNIT_POT-initial_zoom }
                , IntExp { val: Integer::from(location_delta.1), exp: -PIXELS_PER_UNIT_POT-initial_zoom }
                , initial_zoom
            );

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
                (location.0.clone() + location_delta.0).set_precision(PIXELS_PER_UNIT_POT+initial_zoom+zoom_delta)
                , (location.1.clone() + location_delta.1).set_precision(PIXELS_PER_UNIT_POT+initial_zoom+zoom_delta)
                , initial_zoom + zoom_delta
            )
            , serial_number: 1
        };




        let mut source_view = View::new_known(stencil_A.clone(), 0);
        let mut sparse_source_view = SparseView::new(stencil_A.clone());

        for seat in 0..resolution.0*resolution.1 {
            source_view.data[seat]=seat as i32;
            sparse_source_view.insert((seat as i32, stencil_A.seat_and_row(seat)));
        }

        let mut control_view = View::new(stencil_B.clone(), 0);
        control_view.fill_from(&source_view);
        let control_view: SparseView<i32> = control_view.into();

        let mut sparse_view = SparseView::new(stencil_B.clone());
        sparse_view.update_from(&sparse_source_view);

        prop_assert_eq!(control_view, sparse_view);
    }
}