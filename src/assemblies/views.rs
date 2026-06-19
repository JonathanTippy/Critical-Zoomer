// See comment at end

use std::cmp::Ordering;
use std::time::Instant;
use rug::Integer;
use crate::assemblies::structs::*;
use crate::constants::*;
use crate::utils::*;

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
    pub fn index_trust_input(&self, seat_and_row: (isize, isize)) -> usize {
        /*assert!(
            seat_and_row.0 >= 0 && seat_and_row.0 < self.resolution.0 as isize
                && seat_and_row.1 >= 0 && seat_and_row.1 < self.resolution.1 as isize
            , "Index Failure: nonexistent seat."
        );*/
        seat_and_row.1 as usize * self.resolution.0 + seat_and_row.0 as usize
    }
    pub fn clamp_seat_and_row(&self, seat_and_row: (isize, isize)) -> (isize, isize) {
        return (
            seat_and_row.0.clamp(0, self.resolution.0 as isize - 1)
            , seat_and_row.1.clamp(0, self.resolution.1 as isize - 1)
        );
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

                for row in 0..self.stencil.resolution.1 {
                    for seat in 0..self.stencil.resolution.0 {
                        let preferred_source_seat_row = (
                            seat as isize + pan_pixel_delta.0
                            , row as isize + pan_pixel_delta.1
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

                        let represented = preferred_source_seat_row == clamped_source_seat_row;
                        let value = source.data[source_index];
                        let source_alignment = source.bitmap[source_index];
                        let est = represented && source_alignment & EST == EST;
                        let exact = represented && source_alignment & EXACT == EXACT;

                        let source_real_alignment = { if exact { EXACT } else { 0 } } + { if est { EST } else { 0 } };
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
                        pan_self_pixel_delta.0 - (pan_source_pixel_delta.0 << screenspace_delta.2)
                        , pan_self_pixel_delta.1 - (pan_source_pixel_delta.1 << screenspace_delta.2)
                    );

                    let frequency = 1 << screenspace_delta.2;

                    for row in 0..self.stencil.resolution.1 {
                        for seat in 0..self.stencil.resolution.0 {
                            // smaller pixels inherit top left larger pixel
                            let preferred_source_seat_row = (
                                (seat as isize + pan_self_pixel_delta.0) >> screenspace_delta.2
                                , (row as isize + pan_self_pixel_delta.1) >> screenspace_delta.2
                            );
                            // smaller pixels inherit closest larger pixel, bias top left on ties.
                            /*let preferred_source_seat_row = (
                                (seat as isize + pan_self_pixel_delta.0 + (frequency >> 1) - 1) >> screenspace_delta.2
                                , (row as isize + pan_self_pixel_delta.1 + (frequency >> 1) - 1) >> screenspace_delta.2
                            );*/

                            let aligned = (seat as isize - phase.0) % frequency == 0
                                && (row as isize - phase.1) % frequency == 0;

                            let clamped_source_seat_row = source
                                .stencil
                                .clamp_seat_and_row(preferred_source_seat_row);

                            let represented = preferred_source_seat_row == clamped_source_seat_row;
                            let value = source.data[source.stencil.index_trust_input(clamped_source_seat_row)];
                            let source_old_alignment = source.bitmap[source.stencil.index_trust_input(clamped_source_seat_row)];
                            let est = source_old_alignment & EST == EST && represented && aligned;
                            let exact = aligned && represented && source_old_alignment & EXACT == EXACT;

                            let source_alignment = { if exact { EXACT } else { 0 } } + { if est { EST } else { 0 } };
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

                            let represented = preferred_source_seat_row == clamped_source_seat_row;
                            let value = source.data[source.stencil.index_trust_input(clamped_source_seat_row)];
                            let source_alignment = source.bitmap[source.stencil.index_trust_input(clamped_source_seat_row)];
                            let exact = represented && source_alignment & EXACT == EXACT;
                            let est = represented && source_alignment & EST == EST;

                            let source_real_alignment = { if exact { EXACT } else { 0 } } + { if est { EST } else { 0 } };
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
        bitmap: vec!(EXACT + EST, EXACT + EST, EXACT + EST, EXACT + EST)

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
        bitmap: vec!(EXACT + EST, EXACT + EST, EXACT + EST, EXACT + EST),

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
        bitmap: vec!(EST, EST, EST, EST),
        
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
        bitmap: vec!(EXACT + EST, EXACT + EST, EXACT + EST, EXACT + EST),
        
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
        bitmap: vec!(EXACT + EST, EXACT + EST, EXACT + EST, EXACT + EST),
        
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
        bitmap: vec!(EXACT + EST, EST, EST, EST),
        
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
        bitmap: vec!(EXACT + EST, EXACT + EST, EXACT + EST, EXACT + EST, EXACT + EST, EXACT + EST, EXACT + EST, EXACT + EST, EXACT + EST,),

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
        bitmap: vec!(EXACT + EST, EST, EXACT + EST, EST, EST, EST, EST, EXACT + EST, EST, EXACT + EST),

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
        bitmap: vec!(EXACT + EST, EXACT + EST, EXACT + EST, EXACT + EST),
        
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
        bitmap: vec!(EXACT + EST, 0, 0, 0),
        
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
        bitmap: vec!(EXACT + EST, EXACT + EST, EXACT + EST, EXACT + EST),
        
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
        bitmap: vec!(0, 0, EXACT + EST, 0),
        
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
        bitmap: vec!(EXACT + EST, EXACT + EST, EXACT + EST, EXACT + EST),
        
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
        bitmap: vec!(EST, EST, EST, EXACT + EST),
        
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
    #[test]
    fn zoom_in_associativity_test(
        location in (i128::MIN..i128::MAX, i128::MIN..i128::MAX)
        , resolution in (1usize..=100, 1usize..=100)
        , initial_zoom in -1i32<<15..1i32<<15
        //, zoom_direction in prop::sample::select(vec![-1i32, 1i32])
        , zoom_delta_A in 0i32..7i32
        , zoom_delta_B in 0i32..7i32
    ) {

        //let zoom_delta_A = zoom_delta_A * zoom_direction;
        //let zoom_delta_B = zoom_delta_B * zoom_direction;

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
