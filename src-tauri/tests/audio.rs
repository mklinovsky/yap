use std::f32::consts::TAU;
use std::io::Cursor;

use yap_lib::audio::encode_flac;

struct Decoded {
    channels: u32,
    sample_rate: u32,
    bits_per_sample: u32,
    samples: Vec<i32>,
}

fn decode(flac: Vec<u8>) -> Decoded {
    let mut reader = claxon::FlacReader::new(Cursor::new(flac)).unwrap();
    let info = reader.streaminfo();
    Decoded {
        channels: info.channels,
        sample_rate: info.sample_rate,
        bits_per_sample: info.bits_per_sample,
        samples: reader.samples().map(Result::unwrap).collect(),
    }
}

fn tone(frequency: f32, rate: u32, seconds: f32) -> Vec<f32> {
    (0..(rate as f32 * seconds) as usize)
        .map(|i| 0.5 * (TAU * frequency * i as f32 / rate as f32).sin())
        .collect()
}

// Skips the edges, where the resampler's filter has only half a window of signal.
fn rms(samples: &[i32]) -> f64 {
    let middle = &samples[samples.len() / 10..samples.len() * 9 / 10];
    let sum: f64 = middle.iter().map(|&s| (s as f64).powi(2)).sum();
    (sum / middle.len() as f64).sqrt()
}

const HALF_SCALE_SINE_RMS: f64 = 0.5 * 32767.0 / std::f64::consts::SQRT_2;

// FLAC has no valid encoding for streams shorter than 16 samples.
#[test]
fn mono_16khz_input_becomes_16bit_flac_with_clipping() {
    let decoded = decode(encode_flac(&[0.0, 0.5, -1.0, 1.5].repeat(4), 16_000, 1));

    assert_eq!(
        (
            decoded.channels,
            decoded.sample_rate,
            decoded.bits_per_sample,
            decoded.samples
        ),
        (1, 16_000, 16, [0, 16384, -32767, 32767].repeat(4))
    );
}

#[test]
fn stereo_input_is_averaged_to_mono() {
    let decoded = decode(encode_flac(&[0.5, -0.5, 1.0, 0.0].repeat(8), 16_000, 2));

    assert_eq!(
        (decoded.channels, decoded.samples),
        (1, [0, 16384].repeat(8))
    );
}

#[test]
fn one_second_at_44_1khz_becomes_one_second_at_16khz() {
    let decoded = decode(encode_flac(&tone(440.0, 44_100, 1.0), 44_100, 1));

    assert_eq!(
        (decoded.sample_rate, decoded.samples.len()),
        (16_000, 16_000)
    );
}

#[test]
fn speech_band_tone_keeps_its_level_when_downsampled() {
    let decoded = decode(encode_flac(&tone(1_000.0, 48_000, 1.0), 48_000, 1));

    let level = rms(&decoded.samples) / HALF_SCALE_SINE_RMS;
    assert!((0.98..1.02).contains(&level), "level {level}");
}

#[test]
fn tone_above_8khz_is_filtered_out_instead_of_aliasing() {
    let decoded = decode(encode_flac(&tone(12_000.0, 48_000, 1.0), 48_000, 1));

    let level = rms(&decoded.samples) / HALF_SCALE_SINE_RMS;
    assert!(level < 0.01, "level {level}");
}
