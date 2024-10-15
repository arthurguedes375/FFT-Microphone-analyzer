use std::{
    io::{stdout, Write},
    sync::{Arc, Mutex},
    time::Duration,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    StreamConfig,
};
use num_complex::Complex;
use rustfft::num_traits::real::Real;
use sdl2::{event::Event, keyboard::Keycode, pixels::Color, rect::Rect};

struct NoteStatus {
    pub key_number: f32,
    pub raw_note_number: f32,
    pub note_number: f32,
    pub error_percentage: u8,
}

impl NoteStatus {
    fn new(frequency_in_hz: f32) -> Self {
        let key_number = Self::frequency_to_key_number(frequency_in_hz);
        let raw_note_number = Self::key_to_raw_note_number(key_number);
        let note_number = Self::key_to_raw_note_number(key_number.round());
        let error_percentage = Self::get_error_percentage(raw_note_number, note_number);

        Self {
            key_number,
            raw_note_number,
            note_number,
            error_percentage,
        }
    }

    /*
     * Gets the frequency in Hz and returns the corresponding key number on the keyboard.
     * Returns 1 for C1, 2 for C#, 49 for C4, etc...
     */
    fn frequency_to_key_number(freq: f32) -> f32 {
        12.0 * (freq / 440.0).log2() + 49.0
    }

    /**
     * This get's a key that might go from 1 until around 96
     * and returns a number ranging from 1 to 12.
     * 1 being C
     * 2 being C#
     * 3 being D
     * and so on...
     */
    fn key_to_raw_note_number(key: f32) -> f32 {
        ((key - 1.0) % 12.0) - 2.0
    }

    /**
     * Gets a key that ranges from 1 until 12
     * and returns the corresponding name
     */
    fn note_number_to_name(key: f32) -> String {
        let notes_names: [&str; 12] = [
            "C ", "C#", "D ", "D#", "E ", "F ", "F#", "G ", "G#", "A ", "A#", "B ",
        ];
        notes_names[(key - 1.0) as usize].into()
    }

    fn get_error_percentage(raw_note_number: f32, target_note_number: f32) -> u8 {
        ((raw_note_number - target_note_number) * 100.0).round() as u8
    }

    /**
     * Gets the bin index and return the Real World frequency in Hz
     */
    fn bin_index_to_frequency_in_hz(
        bin_index: usize,
        total_bins_len: usize,
        sample_rate: u32,
    ) -> f32 {
        (bin_index as f32 * sample_rate as f32) / total_bins_len as f32
    }

    /**
     * Gets a key number that might range from 1 to around 96
     * and returns the octave that the key belongs to.
     */
    fn get_octave_by_key_number(key_number: f32) -> u8 {
        ((key_number.round() / 12.0).floor() + 1.0) as u8
    }
}

struct Graph {
    // Some state
    max_displayed_frequency: usize,
    data_buffer: Vec<f32>,
    data_locker: Arc<Mutex<Vec<f32>>>,
    paused: Arc<Mutex<bool>>,
    mouse_x: Arc<Mutex<i32>>,
}

fn main() {
    let host = cpal::default_host();
    let mic = host.default_input_device().unwrap();

    let stream_sample_rate = 44100;
    let buffer_size = 2usize.pow(12); // == 4096. Writing like this makes sure that it's a power of two

    // internal buffer
    let fft_transform_buffer = Arc::new(Mutex::new(Vec::<f32>::with_capacity(buffer_size)));

    // Result Buffer containing the FFT of the data
    let fft_transform = Arc::new(Mutex::new(Vec::<f32>::new()));

    let fft_stream = fft_transform.clone();
    let fft_buffer_stream = fft_transform_buffer.clone();

    let stream = mic
        .build_input_stream(
            &StreamConfig {
                channels: 1,
                buffer_size: cpal::BufferSize::Default,
                sample_rate: cpal::SampleRate(stream_sample_rate),
            },
            move |data: &[f32], __info| {
                let mut buf = fft_buffer_stream.lock().unwrap();
                let mut remaining = vec![];

                let sum_data = buf.len() + data.len();

                // If the current data + the buf.len() will overflow the buffer then it
                // appends the max amount data in the buffer and saves the remaining to append to the
                // next DFT run
                if buf.len() < buffer_size && sum_data >= buffer_size {
                    let max_i = data.len() - (sum_data - buffer_size);
                    if max_i > 0 {
                        buf.append(&mut data[0..max_i].to_vec());
                        remaining = data[max_i..].to_vec();
                    }
                }

                // If the buffer is in it's desired size, performs the fft and sends it to the
                // result_buffer
                if buf.len() == buffer_size {
                    let mut output = ndarray::Array1::<Complex<f32>>::from_iter(
                        buf.iter().map(|x| Complex::from(x)),
                    );
                    rustfft::FftPlanner::new()
                        .plan_fft_forward(output.len())
                        .process(output.as_slice_mut().unwrap());

                    let mut result = fft_stream.lock().unwrap();
                    *result = output.iter().map(|x| x.norm()).collect();
                    *buf = remaining;
                } else {
                    // If the buffer is not yet full, just appends it and goes to the next samples
                    buf.append(&mut data.to_vec());
                }
            },
            |error| panic!("Error: {:#?}", error),
            None,
        )
        .unwrap();

    println!("Using device {}", mic.name().unwrap());
    println!("{:?}", mic.default_input_config());

    stream.play().unwrap();

    // SDL Config
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window("Frequency Analyzer", 1500, 600)
        .resizable()
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();

    // Some state
    let max_displayed_frequency = 3000;
    let mut data = vec![];
    let mut paused = false;
    let mut mouse_x = 0;

    'running: loop {
        struct WindowSize {
            width: u32,
            height: u32,
        }
        let window_size = canvas.window().size();
        let window_size = WindowSize {
            width: window_size.0,
            height: window_size.1,
        };

        if !paused {
            let locker = fft_transform.lock().unwrap();
            data = (*locker).clone();
        }

        // Gets the min number of bins required to be able to display
        // the max desired frequency in Hz
        let max_bins_displayed_len =
            (max_displayed_frequency * data.len()) / stream_sample_rate as usize;
        let subset_bins = &data[0..max_bins_displayed_len];

        // Gets some graph dimensions
        let frequency_bar_width = (window_size.width as f64 / max_bins_displayed_len as f64) as i32;
        let padding_top = 10;
        let ground_y = 30;
        if data.len() < buffer_size {
            continue 'running;
        }
        let highest_amplitude_bin = data
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::P),
                    ..
                } => {
                    paused = !paused;
                }
                Event::MouseMotion { x, .. } => {
                    if x >= frequency_bar_width * max_bins_displayed_len as i32 {
                        continue;
                    }

                    mouse_x = x;
                }
                _ => {}
            }
        }

        let analyizing_bin_index =
            (mouse_x / frequency_bar_width) as usize % max_bins_displayed_len;

        let real_frequency = NoteStatus::bin_index_to_frequency_in_hz(
            analyizing_bin_index,
            data.len(),
            stream_sample_rate,
        );

        let note_status = NoteStatus::new(real_frequency);
        /* let key = NoteStatus::frequency_to_key_number(real_frequency);
        let raw_note = NoteStatus::key_to_raw_note_number(key);
        let note = NoteStatus::key_to_raw_note_number(key.round());
        let error_percentage = NoteStatus::get_error_percentage(raw_note, note); */

        print!(
                // "{esc}[2J{esc}[1;1H Buffer len: {buf_len}; max[{bin_index}]: {freq:10.2}Hz ({note}{octave}). Out of tune: {error_percentage}%",
                "\r Buffer_len: {:6} Freq[{analyizing_bin_index:4}]: {real_frequency:10.2}Hz ({note}{octave}). Out of tune: {:4}%{fix_line}",
                data.len(),
                note_status.error_percentage,
                note = NoteStatus::note_number_to_name(note_status.note_number),
                octave= NoteStatus::get_octave_by_key_number(note_status.key_number),
                fix_line = (0..10).map(|_| " ").collect::<Vec<&str>>().join("")
            );
        stdout().flush().unwrap();

        // Rendering:
        // Clears the screen
        canvas.set_draw_color(Color::RGB(30, 30, 30));
        canvas.clear();

        canvas.set_draw_color(Color::RGBA(200, 100, 100, 255));

        for (i, data) in subset_bins.iter().enumerate() {
            let frequency_bar_height = ((window_size.height - ground_y - padding_top) as f32 * data
                / (highest_amplitude_bin.1 * 1.1)) as u32;
            canvas
                .draw_rect(Rect::new(
                    frequency_bar_width * i as i32,
                    (window_size.height - ground_y - frequency_bar_height) as i32,
                    frequency_bar_width as u32,
                    frequency_bar_height,
                ))
                .unwrap();
        }

        canvas.present();

        std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 20));
    }
}
