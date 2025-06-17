use rug::Integer;
use std::{
    cmp::{min,max}
};

use std::ops::{Add, Sub, Mul};

const FP_LOCATION:u8 = 60; // leaves 63-n bits for integers
const ESC_RAD:u8 = 2;
const ESC_RAD_SQR:u8 = ESC_RAD*ESC_RAD;

const SCREEN_MIN_RAD:u8 = 2; // range visible in viewport at zoom of 1

const TWO: FixedPoint = FixedPoint { val:2 * (1<<FP_LOCATION)};
#[derive(Debug, Copy, Clone, PartialEq)]

pub enum Esc {
    In
    , Esc
    , Hlf
    , Exp
    , Unk
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct FixedPoint { //SFP = Small Fixed Point
    val: i64
}

impl Add for FixedPoint {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            val: self.val + other.val
        }
    }
}

impl Sub for FixedPoint {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            val: self.val - other.val
        }
    }
}

impl Mul for FixedPoint {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        Self {
            val: ((self.val as i128 * other.val as i128) >> FP_LOCATION) as i64
        }
    }

}
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MandelComplexRange {
    real_upper_bound: f64
    , real_lower_bound: f64
    , imag_upper_bound: f64
    , imag_lower_bound: f64
}
impl Add for MandelComplexRange {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
              real_upper_bound: self.real_upper_bound + other.real_upper_bound
            , real_lower_bound: self.real_lower_bound + other.real_lower_bound
            , imag_upper_bound: self.imag_upper_bound + other.imag_upper_bound
            , imag_lower_bound: self.imag_lower_bound + other.imag_lower_bound
        }
    }
}

impl Sub for MandelComplexRange {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            real_upper_bound: self.real_upper_bound - other.real_lower_bound
            , real_lower_bound: self.real_lower_bound - other.real_upper_bound
            , imag_upper_bound: self.imag_upper_bound - other.imag_lower_bound
            , imag_lower_bound: self.imag_lower_bound - other.imag_upper_bound
        }
    }
}

 // multiplying with:
 // (a + bi) * (c + di)
 // ac - bd + i(ad + bc)

impl Mul for MandelComplexRange {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self {
            real_upper_bound: self.real_upper_bound * other.real_upper_bound - self.imag_lower_bound * other.imag_lower_bound
            , real_lower_bound: self.real_lower_bound * other.real_lower_bound - self.imag_upper_bound * other.imag_upper_bound
            , imag_upper_bound: self.real_upper_bound * other.imag_upper_bound + self.imag_upper_bound * other.real_upper_bound
            , imag_lower_bound: self.real_lower_bound * other.imag_lower_bound + self.imag_lower_bound * other.real_lower_bound
        }
    }
}

fn fmax(a:f64, b:f64) -> f64 {
    if (a>=b) {a} else {b}
}

fn fmin(a:f64, b:f64) -> f64 {
    if (a<=b) {a} else {b}
}

fn fsqr(i:f64) -> f64 {
    i*i
}

// squaring with:
// (a + bi) * (a + bi)
// aa - bb + 2abi

fn sqr(input: MandelComplexRange) -> MandelComplexRange {

    // bounds for square must consider the sign at each turn

    MandelComplexRange{
        real_upper_bound:
            fmax(fsqr(input.real_lower_bound), fsqr(input.real_upper_bound))
         -
                if (input.imag_lower_bound <=0.0&&input.imag_upper_bound>=0.0) {
                    fmin(fmin(fsqr(input.imag_lower_bound), fsqr(input.imag_upper_bound)), 0.0)
                } else {fmin(fsqr(input.imag_lower_bound), fsqr(input.imag_upper_bound))}

        , real_lower_bound:
            if (input.real_lower_bound <=0.0&&input.real_upper_bound>=0.0) {
                fmin(fmin(fsqr(input.real_lower_bound), fsqr(input.real_upper_bound)), 0.0)
            } else {fmin(fsqr(input.real_lower_bound), fsqr(input.real_upper_bound))}
                -
            fmax(fsqr(input.imag_lower_bound), fsqr(input.imag_upper_bound))
        , imag_upper_bound: fmax(fmax(input.real_upper_bound * input.imag_upper_bound, input.real_upper_bound * input.imag_lower_bound)
        , fmax(input.real_lower_bound * input.imag_upper_bound, input.real_lower_bound * input.imag_lower_bound))* 2.0

        , imag_lower_bound: fmin(fmin(input.real_upper_bound * input.imag_upper_bound, input.real_upper_bound * input.imag_lower_bound)
                                 , fmin(input.real_lower_bound * input.imag_upper_bound, input.real_lower_bound * input.imag_lower_bound))* 2.0
    }

}


fn mbrot(z: MandelComplexRange, c: MandelComplexRange) -> MandelComplexRange {
    sqr(z) + c
    //z*z + c
}


fn h(r:f64, i:f64) -> bool {
    //r*r+i*i > ESC_RAD_SQR as f64
    r.abs()>2.0||i.abs()>2.0
}


fn contains(a:&MandelComplexRange, b:&MandelComplexRange) -> bool {
    if a.real_upper_bound >= b.real_upper_bound
        && a.real_lower_bound <= b.real_lower_bound
        && a.imag_upper_bound >= b.imag_upper_bound
        && a.imag_lower_bound <= b.imag_lower_bound {true}
    else {false}
}

fn overlaps(a:&MandelComplexRange, b:&MandelComplexRange) -> bool {
    if (a.real_upper_bound >= b.real_lower_bound && a.real_lower_bound <= b.real_upper_bound)
    && (a.imag_upper_bound >= b.imag_lower_bound && a.imag_lower_bound <= b.imag_upper_bound)
        {true}
    else {false}
}


fn esccheck(z:&MandelComplexRange, oldz:&MandelComplexRange, C:&MandelComplexRange) -> Esc {

    let esc_box = MandelComplexRange {
        real_upper_bound: ESC_RAD as f64
        , real_lower_bound: -(ESC_RAD as f64)
        , imag_upper_bound: ESC_RAD as f64
        , imag_lower_bound: -(ESC_RAD as f64)
    };

    if contains(&esc_box, z) {
        if contains(oldz, z) || contains(C, z) {return Esc::In;} else {return Esc::Unk;}
    } else {
        if contains(z, &esc_box) {return Esc::Exp;} else {
            /*if (z.real_upper_bound * z.real_upper_bound + z.imag_upper_bound * z.imag_upper_bound > 8.0
                && z.real_lower_bound * z.real_lower_bound + z.imag_lower_bound * z.imag_lower_bound > 8.0
            ) {
                return Esc::Esc;
            } else {return Esc::Hlf;}*/
            if !overlaps(z, &esc_box) {
                return Esc::Esc;
            } else {return Esc::Hlf;}
        }
    }

    let a = h(z.real_upper_bound, z.imag_upper_bound);
    let b = h(z.real_upper_bound, z.imag_lower_bound);
    let c = h(z.real_lower_bound, z.imag_upper_bound);
    let d = h(z.real_lower_bound, z.imag_lower_bound);

    if ! (a || b || c || d) {

        if contains(z, oldz) || contains(z, C) {Esc::In} else {Esc::Unk}
    } else {
        if !(a==b && b==c && c==d) {
            Esc::Hlf
        } else {
            if (z.real_upper_bound > ESC_RAD as f64 && z.real_lower_bound < ESC_RAD as f64 && z.imag_upper_bound > ESC_RAD as f64 && z.imag_lower_bound < ESC_RAD as f64) {
                Esc::Exp
            } else {Esc::Esc}
        }
    }
}

fn eval(z:MandelComplexRange) -> (Esc, u64) {
    let c = z.clone();
    let mut oldz = z.clone();
    let mut z = z;
    let mut h = Esc::Unk;
    for i in 0..100 {
        z = mbrot(z, c);
        h = esccheck(&z, &oldz, &c);
        if h==Esc::Esc || h==Esc::Exp || h==Esc::In {
            return (h, i)
        }
        oldz = z;
    }
    //(Esc::In, 100)
    (h, 100)
}

fn get_screen_value(
    zoom:f64, zoom_recip: f64
    , offset_real: f64
    , offset_imag: f64
    , screen_width: usize
    , screen_height: usize
    , min_side_recip:f64
    , pixel_x:usize
    , pixel_y:usize
) -> MandelComplexRange {

    // normalize screen size

    let min_side = min(screen_width, screen_height);

    let screen_screenspace_width:f64 = screen_width as f64 * min_side_recip;
    let screen_screenspace_height:f64 = screen_height as f64 * min_side_recip;

    // create transforms so screen narrow dimension goes from -0.5 to 0.5 instead of 0 to 1

    let screenspace_x_transform = -(screen_screenspace_width * 0.5);
    let screenspace_y_transform = -(screen_screenspace_height * 0.5);

    // calculate screenspace coordinate

    let left_side_screenspace_x:f64 = (pixel_x as f64) * min_side_recip + screenspace_x_transform;
    let top_side_screenspace_y:f64 = (pixel_y as f64) * min_side_recip + screenspace_y_transform;

    // adjust screenspace coordinates

    let left_side_screenspace_x = left_side_screenspace_x * SCREEN_MIN_RAD as f64 * 2.0; // extra 2.0 for -1 to 1 instead of -0.5 to 0.5
    let top_side_screenspace_y = top_side_screenspace_y * SCREEN_MIN_RAD as f64 * 2.0;

    // screenspace coordinates are now ready to be transformed for use

    //first scrunch down towards zero with zoom, then add offset
    // then invert y because pixels march down but up is positive imag
    let left_side_x:f64 = left_side_screenspace_x * zoom_recip + offset_real;
    let bottom_side_y:f64 = -(top_side_screenspace_y * zoom_recip + offset_imag);
    let pixel_girth:f64 = SCREEN_MIN_RAD as f64*2.0*min_side_recip;
    MandelComplexRange{
        real_upper_bound: left_side_x + pixel_girth
        , real_lower_bound: left_side_x
        , imag_upper_bound: bottom_side_y + pixel_girth
        , imag_lower_bound: bottom_side_y
    }
    //(left_side_x, bottom_side_y)
}

pub fn get_screen_values (zoom: f64, offset_real: f64, offset_imag: f64, screen_width: usize, screen_height: usize) -> Vec<MandelComplexRange> {
    let zoom_recip = 1.0 / zoom;
    let min_side_recip = 1.0 / (min(screen_width, screen_height) as f64);
    let mut returned = vec!();
    for y in 0..screen_height {
        for x in 0..screen_width {
            let value = get_screen_value(zoom, zoom_recip, offset_real, offset_imag, screen_width, screen_height, min_side_recip, x, y);
            returned.push(value);
        }
    }
    returned
}

pub fn eval_screen (inputs: Vec<MandelComplexRange>) -> Vec<(Esc, u64)> {
    let mut returned = vec!();
    for input in inputs {
        returned.push(eval(input));
    }
    returned
}

pub fn paint_result (results: Vec<(Esc, u64)>, screen_buffer: &mut Vec<u32>) {
    assert_eq!(results.len(), screen_buffer.len(), "Results Don't match screen buffer size");
    for i in 0..results.len() {
        let shade = (results[i].1*10 & 127)  as u32;
        if results[i].0 == Esc::Esc {screen_buffer[i] = u32::MAX} else {screen_buffer[i] = 0}
        match results[i].0 {
            Esc::In => {screen_buffer[i]=0} //black
            Esc::Esc => {screen_buffer[i]=(128+shade) * (1<<8)+(128+shade)+(128+shade)*(1<<16)} // white
            Esc::Hlf => {screen_buffer[i]=(128+shade) * (1<<8)} // green
            Esc::Exp => {screen_buffer[i]=(128+shade)} // blue
            Esc::Unk => (screen_buffer[i]=(128+shade)*(1<<16)) //red
        }
    }
}



pub fn compute_screen(
    offset_real: &Integer,
    offset_imag: &Integer,
    screen_width: u32,
    screen_height: u32,
    zoom_pot: u32,
    extra_bits: u32,
    max_iter_pot: u32,
    screen_buffer: &mut Vec<u32>,
) {
    let max_iter: u32 = 1 << max_iter_pot;

    let smaller_side: u32 = min(screen_width, screen_height);

    let side_power: u32 =
        31 - smaller_side.leading_zeros();
    let side_pixels: u32 = 1 << side_power;

    let total_precision: u32 = side_power + zoom_pot + extra_bits;

    // make sure the bits POT is even so we can always cleanly get the root
    let mut adjusted = false;
    let extra_bits = if (zoom_pot + side_pixels) & 1 == 0 {
        if extra_bits & 1 == 0 {
            extra_bits
        } else {
            adjusted = true;
            extra_bits + 1
        }
    } else {
        if extra_bits & 1 == 1 {
            extra_bits
        } else {
            adjusted = true;
            extra_bits + 1
        }
    };
    let offset_real = if adjusted {
        Integer::from(offset_real.clone() << 1)
    } else {
        offset_real.clone()
    };
    let offset_imag = if adjusted {
        Integer::from(offset_imag.clone() << 1)
    } else {
        offset_imag.clone()
    };

   /* let mut bits_type: u8 = 0;

    if total_precision > usize::BITS - 3 {
        bits_type = 1;
    } else {
        bits_type = 2;
    }
*/
    let mut recurrence_vec: Vec<Vec<u32>> = vec![vec![0]; 4096];
    let mut recurrence_vec_temp: Vec<usize> = vec![0; max_iter as usize];

    let extra_bits = extra_bits as i128;
    let zoom_pot = zoom_pot as i128;
    let side_power = side_power as i128;
    let side_pixels = side_pixels as i128;
    let screen_width = screen_width as i128;
    let screen_height = screen_height as i128;
    let max_iter_pot = max_iter_pot as i128;
    let offset_real = offset_real.to_i128().unwrap();
    let offset_imag = offset_imag.to_i128().unwrap();
    let max_iter = max_iter as u128;

    let common_denomenator_power = side_power + zoom_pot + extra_bits - 2;

    let common_denomenator_root_power = common_denomenator_power >> 1;

    println!("bits: {}", common_denomenator_power);

    let common_denomenator = 1 << common_denomenator_power;

    let numerator_of_two = (common_denomenator << 1) >> zoom_pot;

    let real_center_adjust = (((screen_width - side_pixels) << extra_bits) >> 1) + numerator_of_two;

    let imag_center_adjust = (((screen_height - side_pixels) << extra_bits) >> 1) + numerator_of_two;

    for this_pixel_real in 0..screen_width {
        let this_pixel_real_numerator =
            ((this_pixel_real << extra_bits) - real_center_adjust) + offset_real;

        for this_pixel_imag in 0..screen_height {
            let this_pixel_imag_numerator =
                ((this_pixel_imag << extra_bits) - imag_center_adjust) + offset_imag;
            compute_pixel(
                this_pixel_real_numerator,
                this_pixel_imag_numerator,
                common_denomenator_power,
                common_denomenator_root_power,
                max_iter_pot,
                max_iter,
                &mut recurrence_vec_temp,
                &mut recurrence_vec,
                &mut screen_buffer[((this_pixel_imag * screen_width) + this_pixel_real) as usize],
            );
        }
    }
}

fn compute_pixel(
    this_pixel_real_numerator: i128,
    this_pixel_imag_numerator: i128,
    common_denomenator_power: i128,
    common_denomenator_root_power: i128,
    max_iterations_power: i128,
    max_iterations: u128,
    recurrence_vec_temp: &mut Vec<usize>,
    recurrence_vec: &mut Vec<Vec<u32>>,
    // in_rememberer_items_pot: usize,
    this_pixel_color: &mut u32,
) {
    let common_denomenator_power_minus_one: i128 = common_denomenator_power - 1;
    /*let numerator_of_four_after_square: T =
    Into::<T>::into(1) << ((common_denomenator_power << 1.into()) + 2.into());*/
    let numerator_of_four:i128 = 1 << (common_denomenator_power + 2);
    let mut x_real_numerator = this_pixel_real_numerator;
    let mut x_imag_numerator = this_pixel_imag_numerator;
    let denom_root_pot: i128 = common_denomenator_power >> 1;
    *this_pixel_color = 0;
    for this_iteration in 0..max_iterations {
        if ! insert_index(
            recurrence_vec_temp,
            recurrence_vec,
            [x_real_numerator as u32, x_imag_numerator as u32],
            4095,
        ) {
            let x_real_numerator_times_root = x_real_numerator >> common_denomenator_root_power;
            let x_imag_numerator_times_root = x_imag_numerator >> common_denomenator_root_power;
            let x_real_numerator_squared =
                x_real_numerator_times_root * x_real_numerator_times_root;
            let x_imag_numerator_squared =
                x_imag_numerator_times_root * x_imag_numerator_times_root;
            if (x_real_numerator_squared + x_imag_numerator_squared) < numerator_of_four {
                x_imag_numerator = ((x_real_numerator_times_root * x_imag_numerator_times_root) << 1)
                    //>> common_denomenator_power_minus_one
                    + this_pixel_imag_numerator;
                x_real_numerator = (x_real_numerator_squared - x_imag_numerator_squared)
                    //>> common_denomenator_power)
                    + this_pixel_real_numerator;
            } else {
                // *period_vec = period_vec_copy;//period_vec.iter_mut().map(|x| *x = [0,0]).count();
                for i in 0..this_iteration {
                    recurrence_vec[recurrence_vec_temp[i as usize]] = vec![0];
                }
                let shade = (((this_iteration % 14) << 4) + 31) as u32;
                *this_pixel_color = shade + (shade << 8) + (shade << 16);
                break;
            }
        } else {
            break;
        }
    }
    //for vec in period_vec.iter_mut() {vec.clear();}
    //period_vec.iter_mut().map(|x| *x = [0,0]).count();
}
fn insert_index(
    recurrence_vec_temp: &mut Vec<usize>,
    list: &mut Vec<Vec<u32>>,
    item: [u32; 2],
    length: usize,
) -> bool {
    let mut return_thing: bool = false;
    let index: usize = item[1] as usize & length;
    for i in 0..((list[index]).len()) {
        if (list[index])[i] != item[0] {
        } else {
            return_thing = true;
            break;
        }
        if i == (list[index].len()) {
            (list[index]).push(item[0]);
            recurrence_vec_temp.push(index);
        }
    }
    return return_thing;
}

