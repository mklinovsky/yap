use std::io::Cursor;

const TARGET_RATE: u32 = 16_000;

pub fn encode_wav(samples: &[f32], sample_rate: u32, channels: u16) -> Vec<u8> {
    let mono = resample(&downmix(samples, channels), sample_rate);
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut out = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut out, spec).expect("in-memory WAV writer");
    for sample in mono {
        let pcm = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        writer.write_sample(pcm).expect("in-memory WAV write");
    }
    writer.finalize().expect("in-memory WAV finalize");
    out.into_inner()
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
    let step = from_rate as f64 / TARGET_RATE as f64;
    let len = (samples.len() as f64 / step) as usize;
    let last = samples.len() - 1;
    (0..len)
        .map(|i| {
            let pos = i as f64 * step;
            let idx = (pos as usize).min(last);
            let frac = (pos - idx as f64) as f32;
            let next = samples[(idx + 1).min(last)];
            samples[idx] + (next - samples[idx]) * frac
        })
        .collect()
}
