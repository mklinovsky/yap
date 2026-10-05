use std::f32::consts::TAU;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use tauri::image::Image;
use tauri::menu::MenuItem;
use tauri::tray::TrayIcon;
use tauri::{AppHandle, Emitter, Runtime};

use crate::dictation::{Cue, Feedback, Snapshot, Status};

pub const HISTORY_CHANGED: &str = "history-changed";
pub const STATUS_CHANGED: &str = "status-changed";

pub struct TrayFeedback<R: Runtime> {
    pub app: AppHandle<R>,
    pub tray: TrayIcon<R>,
    pub status_item: MenuItem<R>,
    pub retry_item: MenuItem<R>,
}

impl<R: Runtime> Feedback for TrayFeedback<R> {
    fn status(&self, snapshot: &Snapshot) {
        let (label, icon, template) = match &snapshot.status {
            Status::Idle => ("Idle".to_string(), dot([0, 0, 0, 255]), true),
            Status::Recording => ("Recording…".to_string(), dot([230, 57, 70, 255]), false),
            Status::Transcribing => ("Transcribing…".to_string(), dot([244, 162, 97, 255]), false),
            Status::Error(message) => (format!("Error: {message}"), warning(), true),
        };
        let _ = self.tray.set_icon(Some(icon));
        let _ = self.tray.set_icon_as_template(template);
        let _ = self.tray.set_tooltip(Some(format!("yap — {label}")));
        let _ = self.status_item.set_text(label);
        let busy = matches!(snapshot.status, Status::Recording | Status::Transcribing);
        let _ = self.retry_item.set_enabled(snapshot.can_retry && !busy);
        let _ = self.app.emit(STATUS_CHANGED, snapshot);
    }

    fn cue(&self, cue: Cue) {
        let frequency = match cue {
            Cue::Start => 880.0,
            Cue::Stop => 660.0,
        };
        std::thread::spawn(move || {
            if let Err(error) = play_tone(frequency) {
                eprintln!("sound cue failed: {error}");
            }
        });
    }

    fn history_changed(&self) {
        let _ = self.app.emit(HISTORY_CHANGED, ());
    }
}

const ICON_SIZE: u32 = 44;

pub fn dot(rgba: [u8; 4]) -> Image<'static> {
    let center = ICON_SIZE as f32 / 2.0;
    let radius = center * 0.6;
    render(rgba, |x, y| {
        (x - center).powi(2) + (y - center).powi(2) <= radius.powi(2)
    })
}

// A warning triangle with a cut-out exclamation mark; drawn as a template so it never reads as
// the red recording dot.
fn warning() -> Image<'static> {
    let (top, left, right, bottom) = ((22.0, 7.0), 5.0, 39.0, 38.0);
    let inside_triangle = |x: f32, y: f32| {
        let edge = |(ax, ay): (f32, f32), (bx, by): (f32, f32)| {
            (bx - ax) * (y - ay) - (by - ay) * (x - ax)
        };
        y <= bottom && edge(top, (left, bottom)) <= 0.0 && edge((right, bottom), top) <= 0.0
    };
    let in_mark = |x: f32, y: f32| {
        let bar = (20.25..=23.75).contains(&x) && (16.0..=28.5).contains(&y);
        let dot = (x - 22.0).powi(2) + (y - 33.0).powi(2) <= 2.1f32.powi(2);
        bar || dot
    };
    render([0, 0, 0, 255], |x, y| {
        inside_triangle(x, y) && !in_mark(x, y)
    })
}

fn render(rgba: [u8; 4], inside: impl Fn(f32, f32) -> bool) -> Image<'static> {
    const SAMPLES: u32 = 4;
    let mut pixels = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let mut hits = 0;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let px = x as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                    let py = y as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
                    if inside(px, py) {
                        hits += 1;
                    }
                }
            }
            let coverage = hits as f32 / (SAMPLES * SAMPLES) as f32;
            pixels.extend_from_slice(&[
                rgba[0],
                rgba[1],
                rgba[2],
                (rgba[3] as f32 * coverage) as u8,
            ]);
        }
    }
    Image::new_owned(pixels, ICON_SIZE, ICON_SIZE)
}

const TONE_SECONDS: f32 = 0.08;

fn play_tone(frequency: f32) -> Result<(), String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("No audio output device")?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => tone_stream::<f32>(&device, config, frequency),
        SampleFormat::I16 => tone_stream::<i16>(&device, config, frequency),
        SampleFormat::I32 => tone_stream::<i32>(&device, config, frequency),
        SampleFormat::U16 => tone_stream::<u16>(&device, config, frequency),
        format => return Err(format!("Unsupported output sample format {format}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_secs_f32(TONE_SECONDS + 0.05));
    Ok(())
}

fn tone_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    frequency: f32,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let rate = config.sample_rate as f32;
    let channels = config.channels as usize;
    let total = (rate * TONE_SECONDS) as usize;
    let mut frame = 0usize;
    device
        .build_output_stream::<T, _, _>(
            config,
            move |data: &mut [T], _| {
                for chunk in data.chunks_mut(channels) {
                    let value = if frame < total {
                        let t = frame as f32 / rate;
                        let fade = (frame.min(total - frame) as f32 / (rate * 0.01)).min(1.0);
                        (t * frequency * TAU).sin() * 0.2 * fade
                    } else {
                        0.0
                    };
                    frame += 1;
                    for sample in chunk {
                        *sample = T::from_sample(value);
                    }
                }
            },
            |error| eprintln!("sound stream error: {error}"),
            None,
        )
        .map_err(|e| e.to_string())
}
