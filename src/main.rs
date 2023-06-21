use critical_zoomer::*;

use minifb::{Key, ScaleMode, Window, WindowOptions};

use std::{
    cmp::min,
    time::{Duration, Instant},
};

use rug::Integer;

// initial settings are stored as const values
const INIT_SCREEN_WIDTH: usize = 640;
const INIT_SCREEN_HEIGHT: usize = 480;
const INIT_ZOOM_POT: u32 = 0; // POT stands for power of two
const INIT_EXTRA_BITS: u32 = 10;
const MIN_MICROSECONDS_PER_LOOP: u64 = 16666;
// extra bits for more precision
// (about 10 is enough but you may need more for more iterations)
const INIT_BAIL_ITER_POT: u32 = 6;
//const INIT_RAM_BYTES_POT: u8 = 24;

// internal stuff
//const recurrence_vec_indexer: usize = 4095;
const MIN_LOOP_DURATION: Duration = Duration::from_micros(MIN_MICROSECONDS_PER_LOOP as u64);
//const LOOPS_PER_SECOND: u64 = 1000000 / MIN_MICROSECONDS_PER_LOOP;
fn main() {
    let mut window = Window::new(
        "Critical Zoomer",
        INIT_SCREEN_WIDTH,
        INIT_SCREEN_HEIGHT,
        WindowOptions {
            resize: true,
            scale_mode: ScaleMode::UpperLeft,
            ..WindowOptions::default()
        },
    )
    .expect("Unable to create window!");

    let mut screen_width = INIT_SCREEN_WIDTH;
    let mut screen_height = INIT_SCREEN_HEIGHT;
    let mut offset_real = Integer::from(0);
    let mut offset_imag = Integer::from(0);
    let mut zoom_pot = INIT_ZOOM_POT;
    let mut extra_bits = INIT_EXTRA_BITS;
    let mut bail_iter_pot = INIT_BAIL_ITER_POT;
    let mut screen_buffer: Vec<u32> = Vec::with_capacity(INIT_SCREEN_WIDTH * INIT_SCREEN_HEIGHT);
    screen_buffer.resize(screen_width * screen_height, 0);
    let mut will_rerender = true;
    let mut render_time = 0.0;

    // window.limit_update_rate(Some(Duration::from_micros(100000)));

    while window.is_open() {
        let smaller_side: u32 = min(screen_width as u32, screen_height as u32);

    let side_power: u32 =
        31 - smaller_side.leading_zeros();
    let side_pixels: u32 = 1 << side_power;

    let total_precision: u32 = side_power + extra_bits;

        let bits = total_precision;

        let time_when_started_loop = Instant::now();

        if will_rerender {
            println!("Computing resolution {} by {}", screen_width, screen_height);

            let time_when_compute_started = Instant::now();
            compute_screen(
                &offset_real,
                &offset_imag,
                screen_width as u32,
                screen_height as u32,
                zoom_pot,
                extra_bits,
                bail_iter_pot,
                //&mut undo_rememberer,
                //in_rememberer_items_POT,
                //&mut in_rememberer,
                &mut screen_buffer,
            );
            println!("done computing");
            render_time = time_when_compute_started.elapsed().as_micros() as f64 / 1000000.0;
            println!(
                "Computation of resolution {} by {} took {} seconds.",
                screen_width,
                screen_height,
                render_time,
            );
            //in_rememberer.iter_mut().map(|x| *x = 0).count();
        }

        window
            .update_with_buffer(&screen_buffer, screen_width, screen_height)
            .unwrap();
        will_rerender = false;

        // we get the new window size, and check if we're gonna need to re-render
        // then we resize the pixels buffer to the right size for the new screen
        // size
        let (new_screen_width, new_screen_height) = window.get_size();
        if screen_width == new_screen_width && screen_height == new_screen_height {
        } else {
            screen_height = new_screen_height;
            screen_width = new_screen_width;
            will_rerender = true;

            let pixel_size_ratio_real: Integer =
                Integer::from((new_screen_width << bits) / (screen_width << bits));
            let pixel_size_ratio_imag: Integer =
                Integer::from((new_screen_height << bits) / (screen_height << bits));
            offset_real = (offset_real * pixel_size_ratio_real) >> bits;
            offset_imag = (offset_imag * pixel_size_ratio_imag) >> bits;
        }
        assert!(screen_width > 0 && screen_height > 0);
        screen_buffer.resize(screen_width * screen_height, 0);

        assert!(bits >= 1);
        let slowdown_offset = (render_time / 0.1).round() as u32;
        let slowdown_offset = if slowdown_offset >= 1 {slowdown_offset} else {1};
        let offset_move = if bits > 6 { 1 << (bits - 6) } else { 1 << (1) };
        let offset_move = offset_move * slowdown_offset;

        window.get_keys().iter().for_each(|key| match key {
            Key::Left => {
                println!("Moving the viewport left.");
                offset_real = Integer::from(&offset_real - offset_move);
                println!("new offset is {}, {}", offset_real, offset_imag);
                will_rerender = true;
            }
            Key::Right => {
                println!("Moving the viewport right.");
                offset_real = Integer::from(&offset_real + offset_move);
                println!("new offset is {}, {}", offset_real, offset_imag);
                will_rerender = true;
            }
            Key::Up => {
                println!("Moving the viewport up.");
                offset_imag = Integer::from(&offset_imag - offset_move);
                println!("new offset is {}, {}", offset_real, offset_imag);
                will_rerender = true;
            }
            Key::Down => {
                println!("Moving the viewport down");
                offset_imag = Integer::from(&offset_imag + offset_move);
                println!("new offset is {}, {}", offset_real, offset_imag);
                will_rerender = true;
            }
            Key::F => {
                println!("Increasing the zoom by a power of two.");
                zoom_pot = zoom_pot + 1;
                offset_real = Integer::from(&offset_real << 1);
                offset_imag = Integer::from(&offset_imag << 1);
                println!("new zoom is 2^{}", zoom_pot);
                will_rerender = true;
            }
            Key::G => {
                //if zoom_pot > 0 {
                println!("Decreasing the zoom by a power of two");
                if zoom_pot > 0 {
                    zoom_pot = zoom_pot - 1;
                    offset_real = Integer::from(&offset_real >> 1);
                    offset_imag = Integer::from(&offset_imag >> 1);
                    will_rerender = true;
                } else {
                    println!("zoom unchanged");
                }
            }
            Key::E => {
                println!("Increasing the extra precison bits");
                extra_bits = extra_bits + 2;
                offset_real = Integer::from(&offset_real << 2);
                offset_imag = Integer::from(&offset_imag << 2);
                will_rerender = true;
            }
            Key::R => {
                if extra_bits > 0 {
                    println!("Decreasing the extra precision bits");
                    extra_bits = extra_bits - 2;
                    offset_real = Integer::from(&offset_real >> 2);
                    offset_imag = Integer::from(&offset_imag >> 2);
                    will_rerender = true;
                } else {
                    println!("Cannot decrease extra bits past 0");
                }
            }
            Key::I => {
                println!("Increasing the bail Iterations by a power of two");
                bail_iter_pot = bail_iter_pot + 1;
                will_rerender = true;
            }
            Key::O => {
                if bail_iter_pot > 0 {
                    println!("Decreasing the bail Iterations by a power of two");
                    bail_iter_pot = bail_iter_pot - 1;
                    will_rerender = true;
                } else {
                    println!("Cannot decrease iterations past 0")
                }
            }
            _ => (),
        });

        /*window.get_keys_released().iter().for_each(|key| match key {
            Key::Left => println!("released left"),
            Key::Right => println!("released right"),
            Key::Up => println!("released up"),
            Key::Down => println!("released down"),
            _ => (),
        });*/
        let time_elapsed = time_when_started_loop.elapsed();

        let remaining_time = MIN_LOOP_DURATION
            .checked_sub(time_elapsed)
            .unwrap_or_else(|| Duration::from_secs(0));

        std::thread::sleep(remaining_time);
    }
}
