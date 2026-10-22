use std::sync::{atomic::AtomicBool, mpsc, Arc};

use crate::{config::AudioOutputMode, services::vad::actor::VadCommand};

/// Detects the active audio output routing across platforms.
///
/// Returns `AudioOutputMode::Headset` if headphones/headset/earphones are active,
/// or `AudioOutputMode::Speaker` otherwise.
pub fn detect_audio_output_mode() -> AudioOutputMode {
    // 1. Cross-platform check: inspect default output device name via CPAL
    if let Some(mode) = check_cpal_default_device_name() {
        return mode;
    }

    // 2. Linux desktop: check PipeWire / PulseAudio sink active port
    #[cfg(all(target_os = "linux", not(target_os = "android")))]
    if let Some(mode) = check_linux_audio_sink_ports() {
        return mode;
    }

    // 3. Android: check wired headset / bluetooth via kernel switch
    #[cfg(target_os = "android")]
    if let Some(mode) = check_android_audio_route() {
        return mode;
    }

    AudioOutputMode::Speaker
}

/// Spawns an event-driven background watcher that monitors hardware audio route changes
/// (e.g. plugging in or unplugging headphones mid-session) and dynamically updates the VAD actor.
pub fn spawn_audio_route_watcher(
    vad_tx: mpsc::Sender<VadCommand>,
    shutdown: Arc<AtomicBool>,
) -> Option<std::thread::JoinHandle<()>> {
    #[cfg(all(target_os = "linux", not(target_os = "android")))]
    {
        Some(spawn_linux_route_watcher(vad_tx, shutdown))
    }
    #[cfg(not(all(target_os = "linux", not(target_os = "android"))))]
    {
        let _ = (vad_tx, shutdown);
        None
    }
}

/// Monitors Linux PipeWire / PulseAudio sink and card events via `pactl subscribe`.
#[cfg(all(target_os = "linux", not(target_os = "android")))]
fn spawn_linux_route_watcher(
    vad_tx: mpsc::Sender<VadCommand>,
    shutdown: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("vox-audio-route-watcher".to_string())
        .spawn(move || {
            use std::{io::BufRead, sync::atomic::Ordering};

            let mut current_mode = detect_audio_output_mode();
            log::info!(
                "[Audio::RouteWatcher] Initialized event-driven route watcher (initial: {:?})",
                current_mode
            );

            while !shutdown.load(Ordering::Relaxed) {
                let mut child = match std::process::Command::new("pactl")
                    .arg("subscribe")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    Ok(c) => c,
                    Err(e) => {
                        log::debug!(
                            "[Audio::RouteWatcher] pactl subscribe unavailable: {}. Standby.",
                            e
                        );
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        continue;
                    }
                };

                let stdout = match child.stdout.take() {
                    Some(s) => s,
                    None => {
                        let _ = child.kill();
                        continue;
                    }
                };

                let reader = std::io::BufReader::new(stdout);
                for line in reader.lines() {
                    if shutdown.load(Ordering::Relaxed) {
                        let _ = child.kill();
                        return;
                    }

                    match line {
                        Ok(text) => {
                            let lower = text.to_lowercase();
                            if lower.contains("sink")
                                || lower.contains("card")
                                || lower.contains("server")
                            {
                                // Allow OS port change to settle
                                std::thread::sleep(std::time::Duration::from_millis(60));
                                let new_mode = detect_audio_output_mode();
                                if new_mode != current_mode {
                                    log::info!(
                                        "[Audio::RouteWatcher] Dynamic hardware route transition: {:?} -> {:?}",
                                        current_mode,
                                        new_mode
                                    );
                                    current_mode = new_mode.clone();
                                    if let Err(e) = vad_tx.send(VadCommand::UpdateAudioMode(new_mode)) {
                                        log::debug!(
                                            "[Audio::RouteWatcher] VAD actor disconnected: {}",
                                            e
                                        );
                                        let _ = child.kill();
                                        return;
                                    }
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }

                let _ = child.kill();
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        })
        .expect("Failed to spawn audio route watcher thread")
}

/// Checks if the CPAL default output device name indicates headphones or a headset.
fn check_cpal_default_device_name() -> Option<AudioOutputMode> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    let default_device = host.default_output_device()?;
    let name = default_device.name().ok()?.to_lowercase();

    if is_headset_keyword(&name) {
        log::info!(
            "[Audio::Route] Detected headset default output device: '{}'",
            name
        );
        return Some(AudioOutputMode::Headset);
    }

    None
}

/// Helper matching common headphone / headset / earphone keywords across OS device names.
fn is_headset_keyword(name: &str) -> bool {
    name.contains("headphone")
        || name.contains("headset")
        || name.contains("earphone")
        || name.contains("airpod")
        || name.contains("earbud")
        || name.contains("buds")
        || name.contains("in-ear")
        || name.contains("bluez")
}

/// Inspects Linux PipeWire / PulseAudio active sink ports and ALSA switch nodes.
#[cfg(all(target_os = "linux", not(target_os = "android")))]
fn check_linux_audio_sink_ports() -> Option<AudioOutputMode> {
    // Check pactl list sinks for active port
    if let Ok(output) = std::process::Command::new("pactl")
        .args(["list", "sinks"])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).to_lowercase();
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("active port:")
                    && (trimmed.contains("headphone") || trimmed.contains("headset"))
                {
                    log::debug!(
                        "[Audio::Route] Linux PipeWire/PulseAudio active port is headset: {}",
                        trimmed
                    );
                    return Some(AudioOutputMode::Headset);
                }
            }
        }
    }

    // Fallback: check Linux kernel headphone jack switch if exposed in sysfs
    if let Ok(content) = std::fs::read_to_string("/sys/class/switch/h2w/state") {
        let val = content.trim();
        if val == "1" || val == "2" {
            log::info!("[Audio::Route] Linux kernel h2w switch state: {}", val);
            return Some(AudioOutputMode::Headset);
        }
    }

    None
}

/// Inspects Android wired headset state via the standard `/sys/class/switch/h2w/state` node.
#[cfg(target_os = "android")]
fn check_android_audio_route() -> Option<AudioOutputMode> {
    if let Ok(content) = std::fs::read_to_string("/sys/class/switch/h2w/state") {
        let val = content.trim();
        if val == "1" || val == "2" {
            log::info!(
                "[Audio::Route] Android wired headset switch active: {}",
                val
            );
            return Some(AudioOutputMode::Headset);
        }
    }
    None
}
