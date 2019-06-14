use cpal;
use failure::{Error, Fail};
use hound::WavReader;
use std::f32::consts::PI;
use std::sync::{Arc, Mutex};
use std::ops::Deref;
use std::thread;
use std::time::Duration;
use std::iter;

#[derive(Fail, Debug)]
enum AudioError {
    #[fail(display = "Failed to get default output device")]
    NoOutputDevice,
    #[fail(display = "WAV sample format not supported by cpal")]
    InputFormatNotSupported,
    #[fail(display = "WAV sample format not supported by output device")]
    OutputNotSupported,
}

fn format_supported(supported: &cpal::SupportedFormat, actual: &cpal::Format) -> bool {
    supported.channels == actual.channels &&
    supported.data_type == actual.data_type &&
    supported.max_sample_rate >= actual.sample_rate &&
    supported.min_sample_rate <= actual.sample_rate
}

pub fn run() -> Result<(), Error> {
    let mut reader = WavReader::open("res/samples/clap.wav")?;

    let input_format = reader.spec();
    let output_format = cpal::Format {
        channels: input_format.channels,
        sample_rate: cpal::SampleRate(input_format.sample_rate),
        data_type: match input_format.sample_format {
            hound::SampleFormat::Float
                if input_format.bits_per_sample == 32 => cpal::SampleFormat::F32,
            hound::SampleFormat::Int
                if input_format.bits_per_sample == 16 => cpal::SampleFormat::I16,
            _ => Err(AudioError::InputFormatNotSupported)?,
        }
    };
    println!("Using wav input format {:#?}", &input_format);
    println!("Trying to use device output format {:#?}", &output_format);

    let device = cpal::default_output_device()
        .ok_or(AudioError::NoOutputDevice)?;
    // check if there exists a format that supports the  wav_output_format
    let mut formats = device.supported_output_formats()?;
    let device_format = formats.find(|f| format_supported(&f, &output_format))
        .ok_or(AudioError::OutputNotSupported)?;
    print!("Matching device ouput format {:#?}", &device_format);

    let format = output_format;
    let event_loop = cpal::EventLoop::new();
    let stream_id = event_loop.build_output_stream(&device, &format)?;
    event_loop.play_stream(stream_id.clone());

    // let sample_rate = format.sample_rate.0 as f32;
    // let mut sample_clock = 0f32;

    // let tone = Arc::new(Mutex::new(440.0_f32));
    // let tone2 = tone.clone();



    // // Produce a sinusoid of maximum amplitude.
    // let mut next_value = move || {
    //     // sample_clock = (sample_clock + 1.0) % sample_rate;
    //     // (sample_clock * (*tone.deref().lock().unwrap()) * 2.0 * PI / sample_rate).sin()
    // };

    let t1 = thread::spawn(move || {
        event_loop.run(move |_, data| {
            match data {
                // format has been determined beforehand, so using the samples with the correct type
                cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::I16(mut buffer) } => {
                    for output_sample in buffer.chunks_mut(format.channels as usize) {
                        let mut input_samples = reader.samples::<i16>().map(|s| s.unwrap()).chain(iter::repeat(0));
                        for out in output_sample.iter_mut() {
                            *out = input_samples.next().unwrap() // samples are interleaved
                        }
                    }
                },
                cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::F32(mut buffer) } => {
                    for output_sample in buffer.chunks_mut(format.channels as usize) {
                        let mut input_samples = reader.samples::<f32>().map(|s| s.unwrap()).chain(iter::repeat(0.0));
                        for out in output_sample.iter_mut() {
                            *out = input_samples.next().unwrap(); // samples are interleaved
                        }
                    }
                },
                _ => (),
            }
        });
    });

    // let t2 = thread::spawn(move || {
    //     loop {
    //         *tone2.deref().lock().unwrap() = 4400.0_f32;
    //         thread::sleep(Duration::from_millis(500));
    //         *tone2.deref().lock().unwrap() = 880.0_f32;
    //         thread::sleep(Duration::from_millis(500));
    //     }
    // });

    t1.join().unwrap();
    // t2.join().unwrap();
    Ok(())
}
