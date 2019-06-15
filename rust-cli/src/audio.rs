use cpal;
use failure::{Error, Fail};
use hound::WavReader;
use std::{io, thread, sync::{Arc, Mutex}, time::Duration};

#[derive(Fail, Debug)]
enum AudioError {
    #[fail(display = "Failed to get default output device")]
    NoOutputDevice,
    #[fail(display = "WAV sample format not supported by cpal")]
    InputFormatNotSupported,
    #[fail(display = "WAV sample format not supported by output device")]
    OutputNotSupported,
    #[fail(display = "WAV sample format must be the same for all samples")]
    FormatMismatch,
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

trait Sample: hound::Sample + Copy {
    fn zero() -> Self;
    fn saturating_add_sample(self, other: Self) -> Self;
}

impl Sample for f32 {
    fn zero() -> Self { 0.0 }
    fn saturating_add_sample(self, other: Self) -> Self {
        let r = self + other;
        if r < -1.0 { return -1.0; }
        if r > 1.0 { return 1.0; }
        return r;
    }
}

impl Sample for i16 {
    fn zero() -> Self { 0 }
    fn saturating_add_sample(self, other: Self) -> Self {
        self.saturating_add(other)
    }
}

struct Sampler<R: io::Read>(WavReader<R>);

impl<R: io::Read> Sampler<R> {
    // if None is returned the reader has ended or failed
    // samples are interleaved per channel
    fn sample<S: Sample>(&mut self) -> Option<S> {
        self.0.samples::<S>().next()
            .and_then(|r| r.ok())
    }
}

struct MultiSampler<R: io::Read> {
    samplers: Vec<Sampler<R>>,
    common_format: cpal::Format,
}


impl<R: io::Read> MultiSampler<R> {
    fn new(format: cpal::Format) -> Self {
        MultiSampler {
            samplers: Vec::new(),
            common_format: format,
        }
    }
    fn add_reader(&mut self, reader: WavReader<R>) -> Result<(), Error> {
        let format = try_spec_to_format(&reader.spec())?;
        println!("Trying to add reader with format {:#?}", &format);
        if self.common_format != format {
            Err(AudioError::FormatMismatch)?;
        }
        let sampler = Sampler(reader);
        self.samplers.push(sampler);
        Ok(())
    }
    fn sample<S: Sample>(&mut self) -> S {
        let mut sum = S::zero();
        for sampler in self.samplers.iter_mut() {
            if let Some(s) = sampler.sample::<S>() {
                sum = sum.saturating_add_sample(s)
            }
            // FIXME: remove sampler from vec otherwise, as its done now
        }
        sum
    }
}

pub fn run() -> Result<(), Error> {
    let reader = WavReader::open("res/samples/kick.wav")?;

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

    let sampler = MultiSampler::new(output_format);
    let shared_sampler_1 = Arc::new(Mutex::new(sampler));
    let shared_sampler_2 = shared_sampler_1.clone();

    let t1 = thread::spawn(move || {
        event_loop.run(move |_, data| {
            match data {
                // format has been determined beforehand, so using the samples with the correct type
                cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::I16(mut buffer) } => {
                    for out in buffer.iter_mut(){
                        // FIXME: unwrap
                        *out = shared_sampler_1.lock().unwrap().sample::<i16>() // samples are interleaved per channel
                    }
                },
                cpal::StreamData::Output { buffer: cpal::UnknownTypeOutputBuffer::F32(mut buffer) } => {
                    for out in buffer.iter_mut() {
                        // FIXME: unwrap
                        *out = shared_sampler_1.lock().unwrap().sample::<f32>() // samples are interleaved per channel
                    }
                },
                _ => (),
            }
        });
    });

    let t2 = thread::spawn(move || {
        loop {
            shared_sampler_2.lock().unwrap()
                .add_reader(WavReader::open("res/samples/kick.wav").unwrap()).unwrap();
            thread::sleep(Duration::from_millis(250));
            shared_sampler_2.lock().unwrap()
                .add_reader(WavReader::open("res/samples/kick.wav").unwrap()).unwrap();
            thread::sleep(Duration::from_millis(250));
            shared_sampler_2.lock().unwrap()
                .add_reader(WavReader::open("res/samples/kick.wav").unwrap()).unwrap();
            thread::sleep(Duration::from_millis(250));
            shared_sampler_2.lock().unwrap()
                .add_reader(WavReader::open("res/samples/kick.wav").unwrap()).unwrap();
            shared_sampler_2.lock().unwrap()
                .add_reader(WavReader::open("res/samples/cowbell.wav").unwrap()).unwrap();
            thread::sleep(Duration::from_millis(250));
        }
    });

    t1.join().unwrap();
    t2.join().unwrap();
    Ok(())
}
