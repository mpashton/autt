// This is based on https://github.com/RustAudio/cpal/blob/master/examples/beep.rs 

use clap::Parser;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, Sample, SizedSample, I24, Stream
};
use lexpr::{
    Value
};
use anyhow::{anyhow, Result};
use ringbuf::{
    traits::{Consumer, Producer, Split, Observer},
    HeapRb,
};
use std::thread;
use indicatif::{ProgressBar, ProgressStyle};
use autkit::scope::*;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(version, about = "sin generator", long_about = None)]
struct Opt {
    /// The audio device to use
    #[arg(long, default_value_t = String::from("default"))]
    device: String,

    // #[arg(long, default_value_t = String::from(""))]
    // ch: String,

    // #[arg(short, long, default_value_t = 440.0)]
    // freq: f32,

    #[arg(short, long, default_value_t = 0.0)]
    dur: f32,

    // #[arg(short, long, default_value_t = 1.0)]
    // ampl: f32,

    #[arg(long, default_value_t = String::from(""))]
    sinout: String,

    #[arg(long, default_value_t = String::from(""))]
    input: String,

    #[arg(long)]
    mon: bool,

    #[arg(long, default_value_t = String::from(""))]
    scope: String,
}

#[derive(Clone)]
struct CmdSinout {
    freq: f32,
    ampl: f32,
    channels: Vec<f32>,
    dur: f32, // 0 = indefinite
}

impl CmdSinout {
    fn new() -> Self {
        Self {
            freq: 440.0,
            ampl: 1.0,
            channels: Vec::new(),
            dur: 0.0,
        }
    }
}

#[derive(Clone)]
struct CmdInput {
    channels: Vec<u8>,
}

impl CmdInput {
    fn new() -> Self {
        Self {
            channels: Vec::new()
        }
    }
}

#[derive(Clone)]
struct CmdScope {
    channels: Vec<u8>,
}

impl CmdScope {
    fn new() -> Self {
        Self {
            channels: Vec::new()
        }
    }
}

enum Command {
    Sinout(CmdSinout),
    Input(CmdInput)
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    let host = cpal::default_host();

    let output_device = if opt.device == "default" {
        host.default_output_device()
    } else {
        host.output_devices()?
            .find(|x| x.name().map(|y| y == opt.device).unwrap_or(false))
    }
    .expect("failed to find output device");
    println!("Output device: {}", output_device.name()?);

    let input_device = if opt.device == "default" {
        host.default_input_device()
    } else {
        host.input_devices()?
            .find(|x| x.name().map(|y| y == opt.device).unwrap_or(false))
    }
    .expect("failed to find input device");
    println!("Input device: {}", input_device.name()?);

    let config = output_device.default_output_config().unwrap();
    println!("Default output config: {config:?}");

    let output_stream: Option<cpal::Stream>;
    let input_stream: Option<cpal::Stream>;

    // --- sinout
    if opt.sinout.len() > 0 {
        //println!("sinout");

        let sinout_cmd = lexpr::from_str(&opt.sinout)?;

        let mut params = parse_sinout(&sinout_cmd)?;

        // if user passes no channel numbers, send the signal to all the channels
        if params.channels.is_empty() {
            params.channels.resize(config.channels() as usize, 1.0);
        }

        output_stream = match config.sample_format() {
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
            cpal::SampleFormat::F32 => Some(run_sinout::<f32>(&output_device, &config.clone().into(), params).unwrap()),
            //cpal::SampleFormat::F64 => run::<f64>(&device, &config.into()),
            sample_format => panic!("Unsupported sample format '{sample_format}'"),
        };
    }

    // --- input module
    if opt.input.len() > 0 {
        //println!("input");
        let input_args = lexpr::from_str(&opt.input)?;
        let input_cmd = parse_input(&input_args)?;

        let config: cpal::StreamConfig = config.into();
        let channel_ct = config.channels as usize;
        let sample_rate = config.sample_rate.0 as f32;
        let ring = HeapRb::<f32>::new(48000 * channel_ct);
        let (mut producer, mut consumer) = ring.split();

        let input_cmd_p = input_cmd.clone();

        let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let mut overrun = false;
            //println!("input buffer {} samples", data.len());
            for frame in data.chunks_exact(channel_ct) {
                for ch in &input_cmd_p.channels {
                    if producer.try_push(frame[*ch as usize]).is_err() {
                        overrun = true;
                    }
                }
                if overrun {
                    eprintln!("output stream fell behind: try increasing latency");
                }
            }
        };

        //println!("building input stream");
        input_stream = Some(input_device.build_input_stream(&config, input_data_fn, err_fn, None).unwrap());
        //println!("built input stream");

        let input_ch_ct = input_cmd.channels.len();

        if opt.mon {
            //println!("mon");
            let pb = ProgressBar::new(100);
            pb.set_style(ProgressStyle::with_template("{bar} {msg}").unwrap());

            thread::spawn(move || {
                loop {
                    let mut buf: Vec<f32> = Vec::new();
                    let buf_sz = 4096;
                    loop {
                        if buf.len() >= buf_sz {
                            break;
                        }
                        if consumer.occupied_len() >= input_ch_ct {
                            let mut frame = [0.0; 64];
                            consumer.pop_slice(&mut frame);
                            buf.push(frame[0]);
                        }
                    }
                    let mut rms: f32 = 0.0;
                    let mut peak: f32 = 0.0;
                    for s in buf {
                        rms += (s * s);
                        let sm = s.abs();
                        if sm > peak { peak = sm; }
                    }
                    let rms = (rms / (buf_sz as f32)).sqrt();
                    pb.set_message(format!("{rms} {peak}"));
                    pb.set_position((rms * 100.0) as u64);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            });
        }

        else if opt.scope.len() > 0 {
            let args = lexpr::from_str(&opt.scope)?;
            let scope_cmd = parse_scope(&args)?;
            let channel_ct = scope_cmd.channels.len();
            let scopectl = Arc::new(Scope::new());
            {
                let mut scope = scopectl.data.lock().unwrap();
                for ch in &scope_cmd.channels {
                    scope.push(ScopeChannel::new(&*format!("ch{}", *ch)));
                }
            }
            let scopectl_p = scopectl.clone();
            thread::spawn(move || {
                loop {
                    let mut buf: Vec<f32> = Vec::new();
                    let buf_sz = 4096;
                    loop {
                        if buf.len() >= (buf_sz * channel_ct) {
                            break;
                        }
                        if consumer.occupied_len() >= input_ch_ct {
                            let mut frame = vec![0.0; input_ch_ct];
                            consumer.pop_slice(&mut frame);
                            for ch in &scope_cmd.channels {
                                buf.push(frame[*ch as usize]);
                            }
                        }
                    }

                    let trigger_index = find_trigger(&buf, 0, channel_ct);

                    // let mut rms: f32 = 0.0;
                    // let mut peak: f32 = 0.0;
                    // let mut i: u32 = 0;
                    // let mut display_samples: Vec<(f32, f32)> = Vec::new();
                    // for s in buf {
                    //     rms += (s * s);
                    //     let sm = s.abs();
                    //     if sm > peak { peak = sm; }
                    //     if i < 512 {
                    //         let point: (f32, f32) = (((i as f32) / sample_rate), s);
                    //         display_samples.push(point);
                    //         // if i < 24 {
                    //         //     println!("sample {} {} {}", i, point.0, point.1);
                    //         // }
                    //     }
                    //     i = i + 1;
                    // }
                    // let rms = (rms / (buf_sz as f32)).sqrt();

                    for ch in &scope_cmd.channels {
                        scopectl_p.data.lock().unwrap()[*ch as usize] 
                            = calc_scope_channel(&buf, *ch as usize, channel_ct, buf_sz, sample_rate, trigger_index);
                        // data.samples = display_samples;
                        // data.peak = peak;
                        // data.rms = rms;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            });
            run_scope(scopectl.clone()); // does not return
        }
    }

    if opt.dur == 0.0 {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    } else {
        std::thread::sleep(std::time::Duration::from_millis((1000.0 * opt.dur) as u64));
    }

    Ok(())
}

fn find_trigger(buf: &[f32], trigger_ch: usize, ch_ct: usize) -> usize {
    let mut prev_sample = buf[trigger_ch];
    let mut i: usize = 0;
    for frame in buf.chunks_exact(ch_ct) {
        let s = frame[trigger_ch];
        if (prev_sample <= 0.0) && (s > 0.0) {
            return i;
        }
        i += 1;
        prev_sample = s;
    }
    0
}

fn calc_scope_channel(buf: &[f32], ch: usize, ch_ct: usize, buf_sz: usize, sample_rate: f32, trigger_idx: usize) -> ScopeChannel {
    let mut d = ScopeChannel::new("");
    let mut i = 0;
    let display_length = 512;
    let last_sample_idx = trigger_idx + display_length;
    for frame in buf.chunks_exact(ch_ct) {
        let s = frame[ch];
        d.rms += (s * s);
        let sm = s.abs();
        if sm > d.peak { d.peak = sm; }
        if i >= trigger_idx && i < last_sample_idx {
            let point: (f32, f32) = ((((i - trigger_idx) as f32) / sample_rate), s);
            d.samples.push(point);
            // if i < 24 {
            //     println!("sample {} {} {}", i, point.0, point.1);
            // }
        }
        i = i + 1;
    }
    d.rms = (d.rms / (buf_sz as f32)).sqrt();
    d
}

fn parse_input(args: &Value) -> Result<CmdInput> {
    let mut cmd = CmdInput::new();
    for_plist(args, |key, val| {
        match key {
            "ch" => {
                for v in val.list_iter().unwrap() {
                    match v {
                        Value::Number(v) => cmd.channels.push(v.as_u64().unwrap() as u8),
                        _ => ()
                    }
                }
            },
            _ => ()
        }
    });

    Ok(cmd)
}

fn parse_scope(args: &Value) -> Result<CmdScope> {
    let mut cmd = CmdScope::new();
    for_plist(args, |key, val| {
        match key {
            "ch" => {
                for v in val.list_iter().unwrap() {
                    match v {
                        Value::Number(v) => cmd.channels.push(v.as_u64().unwrap() as u8),
                        _ => ()
                    }
                }
            },
            _ => ()
        }
    });

    Ok(cmd)
}

fn parse_sinout(args: &Value) -> Result<CmdSinout> {
    let mut cmd = CmdSinout::new();
    let mut channels: Vec<u8> = Vec::new();
    for_plist(args, |key, val| {
        match key {
            "freq" => cmd.freq = val.as_f64().unwrap() as f32,
            "ampl" => cmd.ampl = val.as_f64().unwrap() as f32,
            "dur" => cmd.dur = val.as_f64().unwrap() as f32,
            "ch" => {
                for v in val.list_iter().unwrap() {
                    match v {
                        Value::Number(v) => channels.push(v.as_u64().unwrap() as u8),
                        _ => ()
                    }
                }
            },
            _ => ()
        }
    });
    // set up channels vector
    // it is a list of gains, corresponding to each channel.
    // user passes a list of channel numbers, so set each of these to 1 and leave the rest at 0.
    if channels.len() > 0 {
        channels.sort();
        let lastch = channels[channels.len() - 1];
        cmd.channels.resize((lastch + 1) as usize, 0.0);
        for ch in channels {
            cmd.channels[ch as usize] = 1.0;
        }
    }
    Ok(cmd)
}

fn for_plist<F>(plist: &Value, mut func: F)
    where F: FnMut(&str, &Value)
{
    let mut i = plist.list_iter().unwrap();
    loop {
        match i.next() {
            Some(key) => {
                match *key {
                    Value::Symbol(_) => {
                        match i.next() {
                            Some(val) => func(key.as_symbol().unwrap(), val),
                            None => break
                        }
                    },
                    _ => break
                }
            },
            None => break
        }
    }
}

fn parse_cmd(cmd: &Value, args: &Value) -> Result<Command> {
    match cmd {
        Value::Symbol(s) => match cmd.as_str().unwrap() {
            "sinout" => Ok(Command::Sinout(parse_sinout(args).unwrap())),
            //"fftmon" => Ok(parse_fftmon(args)),
            _ => Err(anyhow!("unknown command {}", *s))
        },
        _ => Err(anyhow!("bad token {}", cmd))
    }
}

pub fn run_sinout<T>(device: &cpal::Device, config: &cpal::StreamConfig, params: CmdSinout) -> Result<cpal::Stream, anyhow::Error>
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
            sinout_cb(data, channels, &params2, &mut next_value)
        },
        err_fn,
        None,
    )?;

    stream.play()?;

    Ok(stream)
}

fn sinout_cb<T>(output: &mut [T], channels: usize, params: &CmdSinout, next_sample: &mut dyn FnMut() -> f32)
where
    T: Sample + FromSample<f32>,
{
    //println!("sinout cb {}", output.len());
    for frame in output.chunks_mut(channels) {
        //let value: T = T::from_sample(next_sample());
        let value = next_sample();
        let mut gaini = params.channels.iter();
        for sample in frame.iter_mut() {
            match gaini.next() {
                None => *sample = (0.0).to_sample(),
                Some(g) => *sample = (value * g).to_sample()
            }
        }
    }
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {err}");
}
