use flacenc::component::BitRepr;
use flacenc::error::Verify;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

const TARGET_RATE: u32 = 16_000;

pub fn encode_flac(samples: &[f32], sample_rate: u32, channels: u16) -> Vec<u8> {
    let pcm: Vec<i32> = resample(&downmix(samples, channels), sample_rate)
        .into_iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i32)
        .collect();
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .expect("default FLAC config");
    let source = flacenc::source::MemSource::from_samples(&pcm, 1, 16, TARGET_RATE as usize);
    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .expect("in-memory FLAC encode");
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream.write(&mut sink).expect("in-memory FLAC write");
    sink.into_inner()
}

fn downmix(samples: &[f32], channels: u16) -> Vec<f32> {
    samples
        .chunks(channels.max(1) as usize)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

fn resample(samples: &[f32], from_rate: u32) -> Vec<f32> {
    if from_rate == TARGET_RATE || samples.is_empty() {
        return samples.to_vec();
    }
    let mut resampler = Fft::<f32>::new(
        from_rate as usize,
        TARGET_RATE as usize,
        1024,
        1,
        FixedSync::Both,
    )
    .expect("FFT resampler for a fixed rate pair");
    let input = InterleavedSlice::new(samples, 1, samples.len()).expect("mono input buffer");
    resampler
        .process_all(&input, samples.len(), None)
        .expect("in-memory resample")
        .take_data()
}
