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

fn try_spec_to_format(spec: &hound::WavSpec) -> Result<cpal::Format, AudioError> {
    Ok(cpal::Format {
        channels: spec.channels,
        sample_rate: cpal::SampleRate(spec.sample_rate),
        data_type: match spec.sample_format {
            hound::SampleFormat::Float
                if spec.bits_per_sample == 32 => cpal::SampleFormat::F32,
            hound::SampleFormat::Int
                if spec.bits_per_sample == 16 => cpal::SampleFormat::I16,
            _ => Err(AudioError::InputFormatNotSupported)?,
        }
    })
}

fn find_supported_format(device: &cpal::Device, format: &cpal::Format)
    -> Result<cpal::SupportedFormat, Error>
{
    Ok(device.supported_output_formats()?
        .find(|f| format_supported(&f, &format))
        .ok_or(AudioError::OutputNotSupported)?)
}

struct Sampler<R: std::io::Read>(WavReader<R>);

impl<R: std::io::Read> Sampler<R> {
    fn sample<S: hound::Sample + Copy>(&mut self, fallback: S) -> S {
        self.0.samples::<S>().next()
            .map(|r| r.unwrap_or(fallback))
            .unwrap_or(fallback)
    }
}

pub fn run() -> Result<(), Error> {
    let reader = WavReader::open("res/samples/clap.wav")?;

    let input_format = reader.spec();
    let output_format = try_spec_to_format(&input_format)?;
    println!("Using wav input format {:#?}", &input_format);
    println!("Trying to use device output format {:#?}", &output_format);

    let device = cpal::default_output_device()
        .ok_or(AudioError::NoOutputDevice)?;
    // check if there exists a format that supports the  wav_output_format
    let device_format = find_supported_format(&device, &output_format)?;
    println!("Matching device ouput format {:#?}", &device_format);

    let event_loop = cpal::EventLoop::new();
    let stream_id = event_loop.build_output_stream(&device, &output_format)?;
    event_loop.play_stream(stream_id.clone());

    let mut sampler = Sampler(reader);

    let t1 = thread::spawn(move || {
        event_loop.run(move |_, data| {
            match data {
                // format has been determined beforehand, so using the samples with the correct type
                cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::I16(mut buffer) } => {
                    for out in buffer.iter_mut(){
                        *out = sampler.sample::<i16>(0) // samples are interleaved per channel
                    }
                },
                cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::F32(mut buffer) } => {
                    for out in buffer.iter_mut() {
                        *out = sampler.sample::<f32>(0.0) // samples are interleaved per channel
                    }
                },
                _ => (),
            }
        });
    });

    t1.join().unwrap();
    Ok(())
}
