use rug::Integer;
use std::{
    cmp::min,
};

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

