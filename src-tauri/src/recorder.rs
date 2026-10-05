use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

use crate::dictation::{Recorder, Recording};

#[derive(Debug, Clone, serde::Serialize)]
pub struct InputDevice {
    pub id: String,
    pub name: String,
}

pub fn input_devices() -> Vec<InputDevice> {
    let Ok(devices) = cpal::default_host().input_devices() else {
        return Vec::new();
    };
    devices
        .filter_map(|device| {
            Some(InputDevice {
                id: device.id().ok()?.to_string(),
                name: device.description().ok()?.name().to_string(),
            })
        })
        .collect()
}

// A saved device that is unplugged (or from another machine) falls back to the system default.
fn input_device(id: Option<&str>) -> Option<cpal::Device> {
    let host = cpal::default_host();
    id.and_then(|id| id.parse::<cpal::DeviceId>().ok())
        .and_then(|id| host.device_by_id(&id))
        .or_else(|| host.default_input_device())
}

struct Active {
    stop: mpsc::Sender<()>,
    thread: JoinHandle<()>,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}

#[derive(Default)]
pub struct CpalRecorder {
    active: Mutex<Option<Active>>,
}

impl Recorder for CpalRecorder {
    fn start(&self, device: Option<&str>) -> Result<(), String> {
        let device = device.map(String::from);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let samples = Arc::new(Mutex::new(Vec::new()));
        let buffer = samples.clone();
        // cpal streams are not Send on every platform, so the stream lives and dies on its own thread.
        let thread = std::thread::spawn(move || match open_stream(device.as_deref(), buffer) {
            Ok((stream, sample_rate, channels)) => {
                let _ = ready_tx.send(Ok((sample_rate, channels)));
                let _ = stop_rx.recv();
                drop(stream);
            }
            Err(error) => {
                let _ = ready_tx.send(Err(error));
            }
        });
        let (sample_rate, channels) = ready_rx
            .recv()
            .map_err(|_| "Microphone thread exited".to_string())??;
        *self.active.lock().unwrap_or_else(|e| e.into_inner()) = Some(Active {
            stop: stop_tx,
            thread,
            samples,
            sample_rate,
            channels,
        });
        Ok(())
    }

    fn stop(&self) -> Recording {
        let Some(active) = self.active.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return Recording {
                samples: Vec::new(),
                sample_rate: 16_000,
                channels: 1,
            };
        };
        let _ = active.stop.send(());
        let _ = active.thread.join();
        let samples =
            std::mem::take(&mut *active.samples.lock().unwrap_or_else(|e| e.into_inner()));
        Recording {
            samples,
            sample_rate: active.sample_rate,
            channels: active.channels,
        }
    }
}

fn open_stream(
    device: Option<&str>,
    buffer: Arc<Mutex<Vec<f32>>>,
) -> Result<(cpal::Stream, u32, u16), String> {
    let device = input_device(device).ok_or("No microphone found")?;
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, buffer),
        SampleFormat::I16 => build::<i16>(&device, config, buffer),
        SampleFormat::I32 => build::<i32>(&device, config, buffer),
        SampleFormat::U16 => build::<u16>(&device, config, buffer),
        format => return Err(format!("Unsupported microphone sample format {format}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, config.sample_rate, config.channels))
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _| {
                let mut buffer = buffer.lock().unwrap_or_else(|e| e.into_inner());
                buffer.extend(data.iter().map(|&s| s.to_sample::<f32>()));
            },
            |error| eprintln!("microphone stream error: {error}"),
            None,
        )
        .map_err(|e| e.to_string())
}
