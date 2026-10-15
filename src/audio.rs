use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    fs::{self, File},
    io::BufWriter,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};
type Writer = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;
pub struct Microphone {
    stream: Option<cpal::Stream>,
    writer: Writer,
    pub started: Instant,
    samples: Arc<AtomicUsize>,
    pub limit: usize,
    error: Arc<Mutex<Option<String>>>,
}
impl Microphone {
    pub fn start(file: &Path) -> Result<Self> {
        let device = cpal::default_host()
            .default_input_device()
            .context("マイクが見つかりません。OSのマイク設定を確認してください。")?;
        let supported = device.default_input_config()?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let limit =
            (config.sample_rate.0 as usize * config.channels as usize * 600).min(50 * 1024 * 1024);
        let spec = hound::WavSpec {
            channels: config.channels,
            sample_rate: config.sample_rate.0,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let writer = Arc::new(Mutex::new(Some(hound::WavWriter::create(file, spec)?)));
        let samples = Arc::new(AtomicUsize::new(0));
        let error = Arc::new(Mutex::new(None));
        fn stream<T: cpal::SizedSample + cpal::Sample>(
            device: &cpal::Device,
            config: &cpal::StreamConfig,
            writer: Writer,
            samples: Arc<AtomicUsize>,
            limit: usize,
            error: Arc<Mutex<Option<String>>>,
        ) -> Result<cpal::Stream>
        where
            i16: cpal::FromSample<T>,
        {
            let stream_error = error.clone();
            Ok(device.build_input_stream(
                config,
                move |data: &[T], _| {
                    let mut guard = writer.lock().unwrap();
                    if let Some(writer) = guard.as_mut() {
                        for sample in data {
                            if samples.load(Ordering::Relaxed) >= limit {
                                break;
                            }
                            if let Err(failure) = writer.write_sample(sample.to_sample::<i16>()) {
                                *error.lock().unwrap() = Some(failure.to_string());
                                break;
                            }
                            samples.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                },
                move |failure| *stream_error.lock().unwrap() = Some(failure.to_string()),
                None,
            )?)
        }
        let input = match format {
            cpal::SampleFormat::F32 => stream::<f32>(
                &device,
                &config,
                writer.clone(),
                samples.clone(),
                limit,
                error.clone(),
            )?,
            cpal::SampleFormat::I16 => stream::<i16>(
                &device,
                &config,
                writer.clone(),
                samples.clone(),
                limit,
                error.clone(),
            )?,
            cpal::SampleFormat::U16 => stream::<u16>(
                &device,
                &config,
                writer.clone(),
                samples.clone(),
                limit,
                error.clone(),
            )?,
            _ => anyhow::bail!("このマイクの音声形式に対応していません。"),
        };
        input.play()?;
        Ok(Self {
            stream: Some(input),
            writer,
            samples,
            limit,
            error,
            started: Instant::now(),
        })
    }
    pub fn should_stop(&self) -> bool {
        self.started.elapsed().as_secs() >= 600
            || self.samples.load(Ordering::Relaxed) >= self.limit
            || self.error.lock().unwrap().is_some()
    }
    pub fn finish(mut self) -> Result<()> {
        self.stream.take();
        if let Some(writer) = self.writer.lock().unwrap().take() {
            writer.finalize()?;
        }
        if let Some(error) = self.error.lock().unwrap().take() {
            anyhow::bail!("録音に失敗しました: {error}");
        }
        anyhow::ensure!(
            self.samples.load(Ordering::Relaxed) > 0,
            "録音データがありません。"
        );
        Ok(())
    }
}
pub struct Player {
    _output: rodio::OutputStream,
    pub sink: rodio::Sink,
}
impl Player {
    pub fn open(file: &Path) -> Result<Self> {
        let output = rodio::OutputStreamBuilder::open_default_stream()?;
        let sink = rodio::Sink::connect_new(output.mixer());
        sink.append(rodio::Decoder::try_from(File::open(file)?)?);
        Ok(Self {
            _output: output,
            sink,
        })
    }
}
pub fn make_music(file: &Path, focus: bool) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 22050,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(file, spec)?;
    let chords = if focus {
        [
            [220.0, 261.63, 329.63],
            [196.0, 246.94, 293.66],
            [174.61, 220.0, 261.63],
            [196.0, 246.94, 329.63],
        ]
    } else {
        [
            [261.63, 329.63, 392.0],
            [220.0, 261.63, 329.63],
            [174.61, 220.0, 261.63],
            [196.0, 246.94, 293.66],
        ]
    };
    for index in 0..22050 * 16 {
        let t = index as f64 / 22050.0;
        let phase = t % 4.0;
        let envelope = (std::f64::consts::PI * phase / 4.0).sin().powi(2);
        let mut sample: f64 = chords[(t / 4.0) as usize]
            .iter()
            .map(|frequency| (std::f64::consts::TAU * frequency * t).sin() * 0.13)
            .sum::<f64>()
            * envelope;
        if focus {
            let beat = t % 0.5;
            sample += (std::f64::consts::TAU * 660.0 * beat).sin() * (-beat * 28.0).exp() * 0.035;
        }
        writer.write_sample((sample.clamp(-1.0, 1.0) * 32767.0) as i16)?;
    }
    writer.finalize()?;
    Ok(())
}
pub fn commit_audio(temporary: &Path, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .context("音声の保存先を確認してください。")?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    let mut source = File::open(temporary)?;
    std::io::copy(&mut source, staged.as_file_mut())?;
    staged.as_file().sync_all()?;
    staged.persist(destination)?;
    fs::remove_file(temporary)?;
    Ok(())
}
