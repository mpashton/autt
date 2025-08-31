// This is based on https://github.com/RustAudio/cpal/blob/master/examples/beep.rs 

use clap::Parser;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, Sample, SizedSample, I24,
};

#[derive(Parser, Debug)]
#[command(version, about = "sin generator", long_about = None)]
struct Opt {
    /// The audio device to use
    #[arg(long, default_value_t = String::from("default"))]
    device: String,

    #[arg(short, long, default_value_t = String::from(""))]
    channels: String,

    #[arg(short, long, default_value_t = 440.0)]
    freq: f32,

    #[arg(short, long, default_value_t = 0.0)]
    dur: f32,

    #[arg(short, long, default_value_t = 1.0)]
    ampl: f32,
}

#[derive(Clone)]
struct Params {
    freq: f32,
    ampl: f32,
    channels: Vec<f32>,
    dur: f32, // 0 = indefinite
}

impl Params {
    fn new() -> Self {
        Self {
            freq: 440.0,
            ampl: 1.0,
            channels: Vec::new(),
            dur: 0.0,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    let host = cpal::default_host();

    let device = if opt.device == "default" {
        host.default_output_device()
    } else {
        host.output_devices()?
            .find(|x| x.name().map(|y| y == opt.device).unwrap_or(false))
    }
    .expect("failed to find output device");
    println!("Output device: {}", device.name()?);

    let config = device.default_output_config().unwrap();
    println!("Default output config: {config:?}");

    let mut params = Params::new();
    params.freq = opt.freq;
    params.dur = opt.dur;
    params.ampl = opt.ampl;

    // set up channels vector
    // it is a list of gains, corresponding to each channel.
    // user passes a list of channel numbers, so set each of these to 1 and leave the rest at 0.
    // if user passes no channel numbers, send the signal to all the channels
    if opt.channels.is_empty() {
        params.channels.resize(config.channels() as usize, 1.0);
    } else {
        params.channels.resize(config.channels() as usize, 0.0);
        let channels: Vec<i32> = opt.channels.split(",").map(|s| s.parse().unwrap()).collect();
        for ch in channels {
            params.channels[ch as usize] = 1.0;
        }
    }

    match config.sample_format() {
        //cpal::SampleFormat::I8 => run::<i8>(&device, &config.into()),
        //cpal::SampleFormat::I16 => run::<i16>(&device, &config.into()),
        //cpal::SampleFormat::I24 => run::<I24>(&device, &config.into()),
        //cpal::SampleFormat::I32 => run::<i32>(&device, &config.into()),
        // cpal::SampleFormat::I48 => run::<I48>(&device, &config.into()),
        //cpal::SampleFormat::I64 => run::<i64>(&device, &config.into()),
        //cpal::SampleFormat::U8 => run::<u8>(&device, &config.into()),
        //cpal::SampleFormat::U16 => run::<u16>(&device, &config.into()),
        // cpal::SampleFormat::U24 => run::<U24>(&device, &config.into()),
        //cpal::SampleFormat::U32 => run::<u32>(&device, &config.into()),
        // cpal::SampleFormat::U48 => run::<U48>(&device, &config.into()),
        //cpal::SampleFormat::U64 => run::<u64>(&device, &config.into()),
        cpal::SampleFormat::F32 => run::<f32>(&device, &config.into(), params),
        //cpal::SampleFormat::F64 => run::<f64>(&device, &config.into()),
        sample_format => panic!("Unsupported sample format '{sample_format}'"),
    }
}

pub fn run<T>(device: &cpal::Device, config: &cpal::StreamConfig, params: Params) -> Result<(), anyhow::Error>
where
    T: SizedSample + FromSample<f32>
{
    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels as usize;

    // Produce a sinusoid.
    let mut sample_clock = 0f32;
    let mut next_value = move || {
        sample_clock = (sample_clock + 1.0) % sample_rate;
        (sample_clock * params.freq * 2.0 * std::f32::consts::PI / sample_rate).sin() * params.ampl
    };

    let err_fn = |err| eprintln!("an error occurred on stream: {err}");

    let params2 = params.clone();
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            write_data(data, channels, &params2, &mut next_value)
        },
        err_fn,
        None,
    )?;
    stream.play()?;

    if params.dur == 0.0 {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    } else {
        std::thread::sleep(std::time::Duration::from_millis((1000.0 * params.dur) as u64));
    }

    Ok(())
}

fn write_data<T>(output: &mut [T], channels: usize, params: &Params, next_sample: &mut dyn FnMut() -> f32)
where
    T: Sample + FromSample<f32>,
{
    for frame in output.chunks_mut(channels) {
        //let value: T = T::from_sample(next_sample());
        let value = next_sample();
        let mut gaini = params.channels.iter();
        for sample in frame.iter_mut() {
            match gaini.next() {
                None => break,
                Some(g) => {
                    *sample = (value * g).to_sample();
                }
            }
        }
    }
}
