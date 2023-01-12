use bloomfilter::Bloom;
use critical_zoomer::*;
use minifb::{Key, ScaleMode, Window, WindowOptions};
use std::time::{Duration, Instant};

const INITIAL_WIDTH: usize = 935;
const INITIAL_HEIGHT: usize = 899;

fn main() {
    let mut window = Window::new(
        "Potato PC Deep Mandelbrot Zoom",
        INITIAL_WIDTH,
        INITIAL_HEIGHT,
        WindowOptions {
            resize: true,
            scale_mode: ScaleMode::UpperLeft,
            ..WindowOptions::default()
        },
    )
    .expect("Unable to create window");

    window.limit_update_rate(Some(Duration::from_micros(16666)));

    let position_real: f64 = -0.217606544216;
    let position_imag: f64 = -1.114348995666;
    let zoom_power_of_two: i128 = 5;
    let extra_bits: i128 = 20;
    let max_iterations_power: i128 = 11;
    let memory_bytes: usize = 3000;

    let max_iterations: i128 = 1 << max_iterations_power;
    let mut screen_buffer: Vec<u32> = Vec::with_capacity(INITIAL_WIDTH * INITIAL_HEIGHT);
    let mut screen_width;
    let mut screen_height;
    let mut bloom_filter_for_periodicity = Bloom::new(
        memory_bytes,
        if max_iterations > 1000 {
            max_iterations
        } else {
            1000
        } as usize,
    );
    while window.is_open() {
        (screen_width, screen_height) = window.get_size();
        /*if new_screen_size.0 > 0 && new_screen_size.1 > 0 {
            (screen_width, screen_height) = new_screen_size
        }*/
        assert!(screen_width > 0 && screen_height > 0);

        screen_buffer.resize(screen_width * screen_height, 0);

        window.get_keys().iter().for_each(|key| match key {
            Key::Left => println!("holding left"),
            Key::Right => println!("holding right"),
            Key::Up => println!("holding up"),
            Key::Down => println!("holding down"),
            _ => (),
        });

        window.get_keys_released().iter().for_each(|key| match key {
            Key::Left => println!("released left"),
            Key::Right => println!("released right"),
            Key::Up => println!("released up"),
            Key::Down => println!("released down"),
            _ => (),
        });

        /*draw_background(
            &mut screen_buffer
        );*/
        let now = Instant::now();
        compute_screen(
            position_real,
            position_imag,
            screen_width as i128,
            screen_height as i128,
            zoom_power_of_two,
            extra_bits,
            max_iterations_power,
            max_iterations,
            //memory_bytes,
            &mut bloom_filter_for_periodicity,
            &mut screen_buffer,
        );
        let elapsed_time = now.elapsed();
        println!(
            "Computation of resolution {} by {} took {}.{} seconds.",
            screen_width,
            screen_height,
            elapsed_time.as_secs(),
            elapsed_time.subsec_micros()
        );
        window
            .update_with_buffer(&screen_buffer, screen_width, screen_height)
            .unwrap();
    }
}
