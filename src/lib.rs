use awint::ExtAwi;
use bloomfilter::Bloom;
use std::cmp::min;
pub fn compute_screen(
    position_real: f64,
    position_imag: f64,
    screen_width: i128,
    screen_height: i128,
    zoom_power_of_two: i128,
    extra_bits: i128,
    max_iterations_power: i128,
    max_iterations: i128,
    //memory_bytes: usize,
    bloom_filter_for_periodicity: &mut Bloom<[i128; 2]>,
    screen_buffer: &mut Vec<u32>,
) {
    let smaller_side = min(screen_width, screen_height);

    let side_power = 127 - smaller_side.leading_zeros() as i128;

    let side_pixels = 1 << side_power;

    println!(
        "Computing resolution {} by {} ({})",
        screen_width,
        screen_height,
        side_pixels
    );

    //let other_bits: u64 = zoom_power_of_two as u64 + extra_bits as u64;

    let mut extra_bits = extra_bits;
    let mut zoom_power_of_two = zoom_power_of_two;

    let total_precision = side_power + zoom_power_of_two + extra_bits;
    if total_precision > 63 {extra_bits = 0;}
    let total_precision = side_power + zoom_power_of_two + extra_bits;
    if total_precision > 63 {zoom_power_of_two = 63 - side_power;}

    let extra_bits = extra_bits;
    let zoom_power_of_two = zoom_power_of_two;

    let common_denomenator_power = side_power + zoom_power_of_two + extra_bits - 2;

    let common_denomenator = 1 << common_denomenator_power;

    let numerator_of_two = (common_denomenator << 1) >> zoom_power_of_two;

    let real_offset =
        numerator_of_two + (((screen_width - side_pixels) << extra_bits) >> 1);

    let imag_offset =
        numerator_of_two + (((screen_height - side_pixels) << extra_bits) >> 1);

    let position_real = (position_real * common_denomenator as f64) as i128;

    let position_imag = (position_imag * common_denomenator as f64) as i128;

    //println!("{}",numerator_of_two);

    for this_pixel_real in 0..screen_width {
        // println!("this pixel is {} and numerator of two is {} and denomenator is {}", this_pixel_real,numerator_of_two,common_denomenator);
        let this_pixel_real_numerator =
            (this_pixel_real << extra_bits) - real_offset + position_real;

        for this_pixel_imag in 0..screen_height {
            let this_pixel_imag_numerator =
                (this_pixel_imag << extra_bits) - imag_offset + position_imag;
            /*println!(
                "computing pixel at {}, {}",
                this_pixel_real_numerator as f64 / common_denomenator as f64,
                this_pixel_imag_numerator as f64 / common_denomenator as f64
            );*/
            compute_pixel(
                this_pixel_real_numerator,
                this_pixel_imag_numerator,
                common_denomenator_power,
                max_iterations_power,
                max_iterations,
                bloom_filter_for_periodicity,
                &mut screen_buffer[((this_pixel_imag * screen_width)
                    + this_pixel_real) as usize],
            );
        }
    }
}

fn compute_pixel(
    this_pixel_real_numerator: i128,
    this_pixel_imag_numerator: i128,
    common_denomenator_power: i128,
    max_iterations_power: i128,
    max_iterations: i128,
    bloom_filter_for_periodicity: &mut Bloom<[i128; 2]>,
    this_pixel_color: &mut u32,
) {
    let common_denomenator_power_minus_one = common_denomenator_power - 1;
    let numerator_of_four_after_square = 1 << ((common_denomenator_power * 2) + 2);
    // let mut cut_short = false;
    let mut x_real_numerator = this_pixel_real_numerator;
    let mut x_imag_numerator = this_pixel_imag_numerator;

    *this_pixel_color = 0;

    for this_iteration in 0..max_iterations {
        if !Bloom::check_and_set(
             bloom_filter_for_periodicity,
               &[x_real_numerator, x_imag_numerator],
           ) {
        let x_real_numerator_squared = x_real_numerator * x_real_numerator;
        let x_imag_numerator_squared = x_imag_numerator * x_imag_numerator;
        if x_real_numerator_squared + x_imag_numerator_squared < numerator_of_four_after_square {
            x_imag_numerator = ((x_real_numerator * x_imag_numerator)
                >> common_denomenator_power_minus_one)
                + this_pixel_imag_numerator;
            x_real_numerator = ((x_real_numerator_squared - x_imag_numerator_squared)
                >> common_denomenator_power)
                + this_pixel_real_numerator;
        } else {
            
//            let shade: u32;
//            if this_iteration > 255 {shade = 255} else {shade = this_iteration as u32};

            //println!("{}",this_iteration);
            let shade = ((this_iteration % 16) * 16) as u32;
            *this_pixel_color = shade + (shade << 8) + (shade << 16);
            break;
        }
            } else {
                break;
            }
    }
    Bloom::clear(bloom_filter_for_periodicity);
}
pub fn draw_background(screen_buffer: &mut Vec<u32>) {
    for pixel in 0..screen_buffer.len() {
        screen_buffer[pixel] = 0;
    }
}
