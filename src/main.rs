use std::{
    f32::consts::PI,
    io::{stdout, Write},
    sync::{Arc, Mutex},
    time::Duration,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    StreamConfig,
};
use ndarray::{s, Array1};
use num_complex::Complex;
use sdl2::{event::Event, keyboard::Keycode, pixels::Color, rect::Rect};

#[derive(Clone)]
struct NoteStatus {
    frequency_in_hz: f32,
    pub key_number: f32,
    pub raw_note_number: f32,
    pub note_number: f32,
    pub error_percentage: i8,
}

impl NoteStatus {
    fn new(frequency_in_hz: f32) -> Self {
        let key_number = Self::frequency_to_key_number(frequency_in_hz);
        let raw_note_number = Self::key_to_raw_note_number(key_number);
        let note_number = Self::key_to_raw_note_number(key_number.round());
        let error_percentage = Self::get_error_percentage(raw_note_number, note_number);

        Self {
            frequency_in_hz,
            key_number,
            raw_note_number,
            note_number,
            error_percentage,
        }
    }

    pub fn get_frequency_in_hz(&self) -> f32 {
        self.frequency_in_hz
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

    fn get_error_percentage(raw_note_number: f32, target_note_number: f32) -> i8 {
        ((raw_note_number - target_note_number) * 100.0).round() as i8
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

/*
 * I designed the code this way because creating a Graph
 * gives you the freedom of having as many graphs with as many implementations of the data
 * underneath it as you want, then you can just copy and paste the bar rendering loop and
 * change it to the second graph.
 *
 * Tho, don't forget to create separate a "data_locker" for each one of the graphs or they will
 * literally just output the same result, since the underlying data will be the same
 */
struct Graph {
    pub width: u32,
    pub height: u32,
    buffer_size: usize,
    max_displayed_frequency: usize,
    data_buffer: Vec<f32>,
    data_locker: Arc<Mutex<Vec<f32>>>,
    paused: Arc<Mutex<bool>>,
    mouse_x: Arc<Mutex<i32>>,
}

struct GraphBar {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub frequency_data: FrequencyData,
}

#[derive(Clone)]
struct FrequencyData {
    pub note_status: NoteStatus,
    pub amplitude_percentage: u8,
    pub analyzing_bin_index: usize,
}

impl Graph {
    pub fn get_buffer_len(&self) -> usize {
        self.data_buffer.len()
    }
    pub fn run(&mut self, stream_sample_rate: u32) -> (Vec<GraphBar>, Option<usize>) {
        {
            let paused = self.paused.lock().unwrap();
            if !(*paused) {
                let locker = self.data_locker.lock().unwrap();
                self.data_buffer = (*locker).clone();
            }
        }

        // Gets the min number of bins required to be able to display
        // the max desired frequency in Hz
        let max_bins_displayed_len =
            (self.max_displayed_frequency * self.data_buffer.len()) / stream_sample_rate as usize;
        let subset_bins = &self.data_buffer[0..max_bins_displayed_len];

        // Gets some graph dimensions
        let frequency_bar_width = (self.width as f64 / max_bins_displayed_len as f64) as i32;
        let padding_top = 10;
        let ground_y = 30;

        // Since the buffer_size may become large, it may take a few seconds or ms to start getting
        // data and because of that it's good to prevent some errors that might rase like
        // "deviding by zero"
        if self.data_buffer.len() < self.buffer_size {
            return (vec![], None);
        }
        let highest_amplitude_bin = self
            .data_buffer
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        let mut bars = vec![];

        for (i, data) in subset_bins.iter().enumerate() {
            let frequency_bar_height = ((self.height - ground_y - padding_top) as f32 * data
                / (highest_amplitude_bin.1 * 1.1)) as u32;
            let real_frequency = NoteStatus::bin_index_to_frequency_in_hz(
                i,
                self.data_buffer.len(),
                stream_sample_rate,
            );

            let note_status = NoteStatus::new(real_frequency);
            bars.push(GraphBar {
                x: frequency_bar_width * i as i32,
                y: (self.height - ground_y - frequency_bar_height) as i32,
                width: frequency_bar_width as u32,
                height: frequency_bar_height,
                frequency_data: FrequencyData {
                    note_status,
                    analyzing_bin_index: i,
                    amplitude_percentage: ((self.data_buffer[i] / highest_amplitude_bin.1) * 100.0)
                        .round() as u8,
                },
            });
        }

        let mouse_x = {
            let mouse_x = self.mouse_x.lock().unwrap();
            *mouse_x
        };

        if mouse_x >= frequency_bar_width * max_bins_displayed_len as i32 {
            return (bars, None);
        }

        let analyzing_bin_index = (mouse_x / frequency_bar_width) as usize % max_bins_displayed_len;

        (bars, Some(analyzing_bin_index))
    }
}

fn is_power_of_two(n: usize) -> bool {
    n != 0 && (n & (n - 1)) == 0
}

fn fft(signal: &Array1<Complex<f32>>) -> Array1<Complex<f32>> {
    let n = signal.len();
    if !is_power_of_two(n) {
        panic!("For this implementation of the FFT, the signal.len() must be a power of 2. You can pad with zeros the signal to reach the closest power of 2");
    }

    if n == 1 {
        return signal.to_owned();
    }

    let even = fft(&signal.slice(s![..;2]).to_owned());
    let odd = fft(&signal.slice(s![1..;2]).to_owned());

    let max_frequency_range = n / 2;

    let mut output = Array1::<Complex<f32>>::zeros(n);

    for k in 0..max_frequency_range {
        let t = Complex::new(0.0, -2.0 * PI * k as f32 / (n as f32)).exp() * odd[k];
        output[k] = even[k] + t;
        output[k + max_frequency_range] = even[k] - t;
    }

    output
}

enum DisplayColors {
    Error,
    Amplitude,
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
                    let output = fft(&ndarray::Array1::<Complex<f32>>::from_iter(
                        buf.iter().map(|x| Complex::from(x)),
                    ));

                    /*
                     * This project was made as a learning resource for the FFT algorithm
                     * My implementation is not even near as performant as
                     * the standard "rustfft" crate. So, in real world applications use the
                     * official "rustfft" crate instead of my "fft" implementation.
                     *
                     * Besides the HUGE difference in performance, the fft crate can calculate the
                     * FFT for buffers of any size. While my implementation only give correct
                     * results when running in a buffer that has a length that is a power of two.
                     *
                     * If you want to see how to use the "rustfft" crate, take a look at their
                     * docs, but if you just want to set it up in this example you can use the
                     * following code instead of my "fft" function and don't forget to remove the
                     * call to the fft in the line above:
                    // This is code is in the version rustfft = "6.2.0"
                    rustfft::FftPlanner::new()
                        .plan_fft_forward(output.len())
                        .process(output.as_slice_mut().unwrap());
                     */
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
    let paused = Arc::new(Mutex::new(false));
    let mouse_x = Arc::new(Mutex::new(0));

    let mut rustfft_graph = Graph {
        data_buffer: vec![],
        data_locker: fft_transform,
        width: canvas.window().size().0,
        height: canvas.window().size().1,
        max_displayed_frequency,
        buffer_size,
        mouse_x: mouse_x.clone(),
        paused: paused.clone(),
    };

    let display_colors = DisplayColors::Amplitude;

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

        rustfft_graph.width = window_size.width;
        rustfft_graph.height = window_size.height;

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
                    let mut p_lock = paused.lock().unwrap();
                    *p_lock = !*p_lock;
                }
                Event::MouseMotion { x, .. } => {
                    let mut m_lock = mouse_x.lock().unwrap();
                    *m_lock = x;
                }
                _ => {}
            }
        }

        let (bars, frequency_data_index) = rustfft_graph.run(stream_sample_rate);

        if let Some(frequency_data_index) = frequency_data_index {
            let frequency_data = &bars[frequency_data_index].frequency_data;
            let analyzing_bin_index = frequency_data.analyzing_bin_index;
            let real_frequency = frequency_data.note_status.get_frequency_in_hz();
            print!(
                "\r Buffer_len: {:6} Amplitude Percentage: {amplitude_percentage} Freq[{analyzing_bin_index:4}]: {real_frequency:10.2}Hz ({note}{octave}). Out of tune: {:4}%{fix_line}",
                rustfft_graph.get_buffer_len(),
                frequency_data.note_status.error_percentage,
                amplitude_percentage=frequency_data.amplitude_percentage,
                note = NoteStatus::note_number_to_name(frequency_data.note_status.note_number),
                octave= NoteStatus::get_octave_by_key_number(frequency_data.note_status.key_number),
                fix_line = (0..10).map(|_| " ").collect::<Vec<&str>>().join("")
            );
            stdout().flush().unwrap();
        }

        // Rendering:
        // canvas.set_draw_color(Color::RGB(30, 30, 30));
        canvas.set_draw_color(Color::RGB(240, 240, 240));
        canvas.clear();

        for bar in bars {
            match display_colors {
                DisplayColors::Error => {
                    let error_gap = 20;
                    if bar.frequency_data.note_status.error_percentage > error_gap {
                        canvas.set_draw_color(Color::RGBA(239, 71, 111, 255));
                    } else if bar.frequency_data.note_status.error_percentage < (-1 * error_gap) {
                        canvas.set_draw_color(Color::RGBA(255, 209, 102, 255));
                    } else {
                        canvas.set_draw_color(Color::RGBA(6, 214, 160, 255));
                    }
                }
                DisplayColors::Amplitude => {
                    let max_red = 200.0;
                    let min_red = 63.0;

                    let max_blue = 184.0;
                    let min_blue = 104.0;
                    let amplitude_percentage =
                        bar.frequency_data.amplitude_percentage as f64 / 100.0;
                    canvas.set_draw_color(Color::RGBA(
                        (amplitude_percentage * (max_red - min_red) + min_red).round() as u8,
                        36,
                        (((1.0 - amplitude_percentage) * (max_blue - min_blue) + min_blue).round()) as u8,
                        255,
                    ));
                }
            }
            canvas
                .fill_rect(Rect::new(bar.x, bar.y, bar.width, bar.height))
                .unwrap();
        }

        canvas.present();

        std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 20));
    }
}
