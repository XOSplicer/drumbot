use cpal;
use failure::{Error, Fail};
use hound::WavReader;
use std::{io, thread, sync::{Arc, Mutex}, path::Path, fs};

#[derive(Fail, Debug)]
enum AudioError {
    #[fail(display = "Failed to get default output device")]
    NoOutputDevice,
    #[fail(display = "WAV sample format not supported by cpal")]
    InputFormatNotSupported,
    #[fail(display = "WAV sample format not supported by output device")]
    OutputNotSupported,
    #[fail(display = "WAV sample format must be the same for all samples")]
    FormatMismatch(cpal::Format, cpal::Format),
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

trait Sample: hound::Sample + Copy + std::ops::Div<Output = Self> + From<i16> {
    fn zero() -> Self;
    fn clipping_add(self, other: Self) -> Self;
    fn is_clipping(self) -> bool;
    fn scale_for(self, i: usize) -> Self {
        self / From::from(i as i16)
    }
}

impl Sample for f32 {
    fn zero() -> Self { 0.0 }
    fn clipping_add(self, other: Self) -> Self {
        let r = self + other;
        if r < -1.0 { return -1.0; }
        if r > 1.0 { return 1.0; }
        return r;
    }
    fn is_clipping(self) -> bool {
        self >= 1.0 || self <= -1.0
    }
}

impl Sample for i16 {
    fn zero() -> Self { 0 }
    fn clipping_add(self, other: Self) -> Self {
        self.saturating_add(other)
    }
    fn is_clipping(self) -> bool {
        self == i16::min_value() || self == i16::max_value()
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
    samplers: Vec<Option<Sampler<R>>>,
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
        // println!("Trying to add reader with format {:#?}", &format);
        if self.common_format != format {
            Err(AudioError::FormatMismatch(self.common_format.clone(), format))?;
        }
        let sampler = Sampler(reader);
        self.samplers.push(Some(sampler));
        Ok(())
    }
    fn sample<S: Sample>(&mut self) -> S {
        let mut sum = S::zero();
        let len = self.samplers.len();
        for sampler in self.samplers.iter_mut() {
            if sampler.is_none() {
                continue;
            }
            if let Some(s) = sampler.as_mut().unwrap().sample::<S>() {
                sum = sum.clipping_add(s.scale_for(len))
            } else {
                *sampler = None;
            }
        }
        self.samplers.retain(|s| s.is_some());
        if sum.is_clipping() {
            print!("!");
        }
        sum
    }
    fn active_samplers(&self) -> usize {
        self.samplers.len()
    }
}

pub fn run() -> Result<AudioEngine, Error> {

    // FIXME: is there a better method of initialiting?
    // --> pass reference file to creation

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

    Ok(AudioEngine {
        event_loop_thread: t1,
        shared_sampler: shared_sampler_2,
    })
}

pub struct AudioEngine {
    event_loop_thread: thread::JoinHandle<()>,
    shared_sampler: Arc<Mutex<MultiSampler<io::BufReader<fs::File>>>>
}

// TODO: implement split() to take AudioEngine apart

impl AudioEngine {
    pub fn join(self) -> thread::Result<()> {
        // this actually never returns
        self.event_loop_thread.join()
    }
    pub fn dispatch_wav<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        let mut lock = self.shared_sampler.lock().unwrap();
        lock.add_reader(WavReader::open(path)?)?;
        println!("Active samplers: {}", lock.active_samplers());
        Ok(())
    }
}

