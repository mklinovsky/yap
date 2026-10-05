use std::io::Cursor;

use yap_lib::wav::encode_wav;

fn decode(wav: Vec<u8>) -> (hound::WavSpec, Vec<i16>) {
    let reader = hound::WavReader::new(Cursor::new(wav)).unwrap();
    let spec = reader.spec();
    let samples = reader.into_samples::<i16>().map(Result::unwrap).collect();
    (spec, samples)
}

#[test]
fn mono_16khz_input_becomes_16bit_pcm_with_clipping() {
    let (spec, samples) = decode(encode_wav(&[0.0, 0.5, -1.0, 1.5], 16_000, 1));

    assert_eq!(
        (
            spec.channels,
            spec.sample_rate,
            spec.bits_per_sample,
            samples
        ),
        (1, 16_000, 16, vec![0, 16384, -32767, 32767])
    );
}

#[test]
fn stereo_input_is_averaged_to_mono() {
    let (spec, samples) = decode(encode_wav(&[0.5, -0.5, 1.0, 0.0], 16_000, 2));

    assert_eq!((spec.channels, samples), (1, vec![0, 16384]));
}

#[test]
fn input_at_48khz_is_downsampled_to_16khz() {
    let (spec, samples) = decode(encode_wav(&[0.0, 0.25, 0.5, 0.75, 1.0, 0.75], 48_000, 1));

    assert_eq!((spec.sample_rate, samples), (16_000, vec![0, 24575]));
}
