//! ============================================================================
//! generate_supertonic_test_clips.rs — Supertonic 3 Audio Clip Generator Utility
//! ============================================================================
//! Category     : Utility Tool (Cargo Example)
//! Component    : Sherpa-ONNX Supertonic 3 TTS
//! Prerequisites: Local Supertonic 3 model at `~/.vox/models/tts/supertonic-3`
//! Execution    : cargo run --release --example generate_supertonic_test_clips
//! ============================================================================

use anyhow::{anyhow, Result};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsSupertonicModelConfig,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

struct ClipPrompt {
    id: &'static str,
    voice_name: &'static str,
    sid: i32,
    text: &'static str,
}

const TEST_PROMPTS: &[ClipPrompt] = &[
    ClipPrompt {
        id: "supertonic_01_en_briefing",
        voice_name: "Watson (Speaker 0)",
        sid: 0,
        text: "Hey Vox, good morning! Can you check my calendar and give me a quick briefing on today's scheduled meetings?",
    },
    ClipPrompt {
        id: "supertonic_02_en_weather",
        voice_name: "Luna (Speaker 1)",
        sid: 1,
        text: "Vox, what's the weather like outside right now? Is it going to rain later this afternoon?",
    },
    ClipPrompt {
        id: "supertonic_03_en_code",
        voice_name: "Moka (Speaker 2)",
        sid: 2,
        text: "Can you help me refactor this Rust async function to reduce mutex contention across our background threads?",
    },
    ClipPrompt {
        id: "supertonic_04_en_summary",
        voice_name: "Nora (Speaker 3)",
        sid: 3,
        text: "Hey Vox, summarize the key action items from my design review notes and draft a quick email to the team.",
    },
    ClipPrompt {
        id: "supertonic_05_en_timer",
        voice_name: "Alphonse (Speaker 4)",
        sid: 4,
        text: "Set a timer for twenty-five minutes for a focused Pomodoro session, and minimize background notifications.",
    },
    ClipPrompt {
        id: "supertonic_06_hi_greeting",
        voice_name: "Luna (Speaker 1)",
        sid: 1,
        text: "हे वॉक्स, नमस्ते! क्या आप मेरा आज का शेड्यूल देखकर बता सकते हैं कि मेरी अगली मीटिंग कब है?",
    },
    ClipPrompt {
        id: "supertonic_07_hi_weather",
        voice_name: "Watson (Speaker 0)",
        sid: 0,
        text: "वॉक्स, आज बाहर का मौसम कैसा है? क्या शाम को बारिश होने की कोई संभावना है?",
    },
    ClipPrompt {
        id: "supertonic_08_hi_reminder",
        voice_name: "Nora (Speaker 3)",
        sid: 3,
        text: "मेरे लिए एक ज़रूरी रिमाइंडर सेट कर दो, शाम को पाँच बजे टीम के साथ प्रोजेक्ट रिव्यू करना है।",
    },
    ClipPrompt {
        id: "supertonic_09_hi_system_cmd",
        voice_name: "Moka (Speaker 2)",
        sid: 2,
        text: "वॉक्स, टर्मिनल खोलिए और हाई परफॉरमेंस मोड ऑन करके लोकल सर्वर शुरू कर दीजिए।",
    },
    ClipPrompt {
        id: "supertonic_10_hi_qa",
        voice_name: "Keld (Speaker 5)",
        sid: 5,
        text: "वॉक्स, मुझे समझाइए कि मशीन लर्निंग में स्पीच-टू-टेक्स्ट मॉडल इतनी तेज़ी से आवाज़ कैसे पहचानते हैं?",
    },
];

/// Writes mono f32 samples to a 16-bit PCM WAV file.
fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &sample in samples {
        let i16_val = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
        writer.write_sample(i16_val)?;
    }
    writer.finalize()?;
    Ok(())
}

fn main() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
    let vox_root = home.join(".vox");
    vox_lib::utils::paths::init_with_root(vox_root);

    let super_tts_path = vox_lib::utils::paths::model_dir("tts").join("supertonic-3");
    if !super_tts_path.exists() {
        return Err(anyhow!("Supertonic 3 model not found at {:?}", super_tts_path));
    }

    let mp = |f: &str| -> String { super_tts_path.join(f).to_string_lossy().into() };

    println!("[Supertonic Generator] Initializing direct OfflineTts engine...");
    let config = OfflineTtsConfig {
        model: OfflineTtsModelConfig {
            supertonic: OfflineTtsSupertonicModelConfig {
                duration_predictor: Some(mp("duration_predictor.int8.onnx")),
                text_encoder: Some(mp("text_encoder.int8.onnx")),
                vector_estimator: Some(mp("vector_estimator.int8.onnx")),
                vocoder: Some(mp("vocoder.int8.onnx")),
                tts_json: Some(mp("tts.json")),
                unicode_indexer: Some(mp("unicode_indexer.bin")),
                voice_style: Some(mp("voice.bin")),
            },
            num_threads: 4,
            debug: false,
            ..Default::default()
        },
        ..Default::default()
    };

    let tts = OfflineTts::create(&config)
        .ok_or_else(|| anyhow!("Failed to instantiate OfflineTts engine"))?;

    println!(
        "[Supertonic Generator] Engine ready. Speakers: {}, Native Sample Rate: {}Hz",
        tts.num_speakers(),
        tts.sample_rate()
    );

    let out_dir = PathBuf::from("/home/addy/projects/apps/vox/sandbox/supertonic_clips");
    fs::create_dir_all(&out_dir)?;

    println!(
        "[Supertonic Generator] Generating clips (Max Quality Steps: 16, Native 44.1kHz Studio Quality)..."
    );

    for prompt in TEST_PROMPTS {
        let out_path = out_dir.join(format!("{}.wav", prompt.id));
        let sid = prompt.sid % tts.num_speakers().max(1);
        print!("  Synthesizing {} (Voice: {}, sid={})... ", prompt.id, prompt.voice_name, sid);

        let lang = if vox_lib::services::translit::is_devanagari(prompt.text) {
            "hi"
        } else {
            "en"
        };
        let mut extra = HashMap::new();
        extra.insert("lang".to_string(), serde_json::json!(lang));

        let gen_config = GenerationConfig {
            sid,
            num_steps: 16, // Maximum quality diffusion steps for studio fidelity
            speed: 1.0,
            silence_scale: 0.1,
            extra: Some(extra),
            ..Default::default()
        };

        let start = std::time::Instant::now();
        let audio = tts.generate_with_config(prompt.text, &gen_config, None::<fn(&[f32], f32) -> bool>);
        let elapsed = start.elapsed();

        if let Some(audio_data) = audio {
            let samples = audio_data.samples();
            write_wav(&out_path, samples, tts.sample_rate() as u32)?;
            let duration_sec = samples.len() as f32 / tts.sample_rate() as f32;
            println!(
                "DONE -> {} ({:.2}s audio, {:.2}s compute, 44.1kHz)",
                out_path.display(),
                duration_sec,
                elapsed.as_secs_f32()
            );
        } else {
            println!("FAILED");
        }
    }

    println!("\n[Supertonic Generator] All studio clips generated successfully!");
    Ok(())
}
