use anyhow::{anyhow, Result};
use ndarray::{s, Array1, Array2, Array4, ArrayView4, Axis};
use ort::session::Session;
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::borrow::Cow;
use std::f32::consts::PI;
use std::path::Path;
use std::sync::Arc;
use tokenizers::{AddedToken, Tokenizer};
use tokenizers::models::bpe::BPE;

// ─── Constants ────────────────────────────────────────────────────────────────

const NUM_MELS: usize = 128;
const FFT_SIZE: usize = 400;    // 25ms at 16kHz
const HOP_LENGTH: usize = 160;  // 10ms at 16kHz
const SAMPLE_RATE: f32 = 16000.0;

// KV cache config (from ONNX model inspection)
const NUM_LAYERS: usize = 28;
const NUM_KV_HEADS: usize = 8;
const HEAD_DIM: usize = 128;
const MAX_TOTAL_LEN: usize = 1024;
const MAX_NEW_TOKENS: usize = 256;

// Token IDs (from tokenizer_config.json)
const IM_START_ID: i64 = 151644;
const IM_END_ID: i64 = 151645;
const EOS_ID: i64 = 151645;  // <|im_end|> is EOS for Qwen3-ASR
const AUDIO_START_ID: i64 = 151669;
const AUDIO_END_ID: i64 = 151670;
const AUDIO_PAD_ID: i64 = 151676;
const ASR_TEXT_ID: i64 = 151704;

// ─── Mel Spectrogram ─────────────────────────────────────────────────────────

pub struct MelSpectrogram {
    num_mels: usize,
    fft_size: usize,
    hop_length: usize,
    mel_filters: Array2<f32>,
    window: Array1<f32>,
    fft: Arc<dyn Fft<f32>>,  // cached FFT plan — not re-created on each call
}

impl MelSpectrogram {
    pub fn new() -> Self {
        let window = Array1::from_shape_fn(FFT_SIZE, |i| {
            0.5 * (1.0 - (2.0 * PI * i as f32 / (FFT_SIZE as f32 - 1.0)).cos())
        });
        let mel_filters = Self::build_mel_filters();

        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        Self {
            num_mels: NUM_MELS,
            fft_size: FFT_SIZE,
            hop_length: HOP_LENGTH,
            mel_filters,
            window,
            fft,
        }
    }

    fn hz_to_mel(hz: f32) -> f32 {
        2595.0 * (1.0 + hz / 700.0).log10()
    }

    fn mel_to_hz(mel: f32) -> f32 {
        700.0 * (10.0f32.powf(mel / 2595.0) - 1.0)
    }

    fn build_mel_filters() -> Array2<f32> {
        let f_min = 0.0_f32;
        let f_max = SAMPLE_RATE / 2.0;
        let mel_min = Self::hz_to_mel(f_min);
        let mel_max = Self::hz_to_mel(f_max);

        let mel_pts = Array1::linspace(mel_min, mel_max, NUM_MELS + 2);
        let hz_pts = mel_pts.mapv(Self::mel_to_hz);
        let bin_pts = hz_pts.mapv(|hz| (FFT_SIZE as f32 + 1.0) * hz / SAMPLE_RATE);

        let n_fft = FFT_SIZE / 2 + 1;
        let mut filters = Array2::zeros((NUM_MELS, n_fft));

        for i in 0..NUM_MELS {
            let left = bin_pts[i] as usize;
            let center = bin_pts[i + 1] as usize;
            let right = bin_pts[i + 2] as usize;

            for j in left..center {
                if j < n_fft && center > left {
                    filters[[i, j]] = (j - left) as f32 / (center - left) as f32;
                }
            }
            for j in center..right {
                if j < n_fft && right > center {
                    filters[[i, j]] = (right - j) as f32 / (right - center) as f32;
                }
            }
            
        }
        filters
    }

    /// Returns [n_frames, NUM_MELS]
    pub fn extract(&self, audio: &[f32]) -> Array2<f32> {
        if audio.len() < self.fft_size {
            return Array2::zeros((0, self.num_mels));
        }
        let n_frames = (audio.len() - self.fft_size) / self.hop_length + 1;
        let n_fft = self.fft_size / 2 + 1;
        let mut out = Array2::zeros((n_frames, self.num_mels));

        for i in 0..n_frames {
            let start = i * self.hop_length;
            let frame = &audio[start..start + self.fft_size];

            let mut buf: Vec<Complex<f32>> = frame
                .iter()
                .zip(self.window.iter())
                .map(|(&s, &w)| Complex::new(s * w, 0.0))
                .collect();

            self.fft.process(&mut buf);

            let power: Vec<f32> = buf[..n_fft].iter().map(|c| c.norm_sqr()).collect();
            let power_arr = Array1::from(power);
            let mel_row = self.mel_filters.dot(&power_arr);

            for j in 0..self.num_mels {
                // Whisper/Qwen-Audio normalization: log10, clip, scale
                // We use 1e-10 as floor, then log10.
                out[[i, j]] = mel_row[j].max(1e-10).log10();
            }
        }

        for x in out.iter_mut() {
            *x = (*x + 4.0) / 4.0;
        }

        out
    }
}

// ─── Commands ─────────────────────────────────────────────────────────────────

pub enum SttCommand {
    /// Partial utterance buffer for real-time feedback.
    Partial(u32, Vec<f32>),
    /// Full utterance buffer from VAD (speech_start → speech_end).
    Final(u32, Vec<f32>),
}

// ─── Engine ───────────────────────────────────────────────────────────────────

pub struct SttEngine {
    conv_frontend: Session,
    encoder: Session,
    decoder: Session,
    tokenizer: Tokenizer,
    mel: MelSpectrogram,
}

fn calculate_audio_token_len(n_frames: usize) -> usize {
    let input_lengths_leave = n_frames % 100;
    let feat_lengths = if input_lengths_leave > 0 {
        (input_lengths_leave - 1) / 2 + 1
    } else {
        0
    };

    let mut output_lengths = if feat_lengths > 0 {
        ((feat_lengths - 1) / 2 + 1 - 1) / 2 + 1
    } else {
        0
    };

    output_lengths += (n_frames / 100) * 13;
    output_lengths
}

impl SttEngine {
    pub fn new(model_dir: &Path) -> Result<Self> {
        // ── Sessions ────────────────────────────────────────────────────────
        let conv_frontend = Session::builder()?
            .with_intra_threads(2)
            .map_err(|e| anyhow!("intra threads: {:?}", e))?
            .commit_from_file(model_dir.join("conv_frontend.onnx"))
            .map_err(|e| anyhow!("load conv_frontend: {:?}", e))?;

        let encoder = Session::builder()?
            .with_intra_threads(2)
            .map_err(|e| anyhow!("intra threads: {:?}", e))?
            .commit_from_file(model_dir.join("encoder.int8.onnx"))
            .map_err(|e| anyhow!("load encoder: {:?}", e))?;

        let decoder = Session::builder()?
            .with_intra_threads(2)
            .map_err(|e| anyhow!("intra threads: {:?}", e))?
            .commit_from_file(model_dir.join("decoder.int8.onnx"))
            .map_err(|e| anyhow!("load decoder: {:?}", e))?;

        for input in decoder.inputs() {
            log::info!("[STT] Decoder Input: {}", input.name());
        }
        for output in decoder.outputs() {
            log::info!("[STT] Decoder Output: {}", output.name());
        }

        // ── Tokenizer ────────────────────────────────────────────────────────
        // The model ships vocab.json + merges.txt (BPE) but no tokenizer.json.
        // We load BPE manually and register special tokens so the tokenizer
        // knows their IDs without splitting them.
        let tok_dir = model_dir.join("tokenizer");
        let vocab = tok_dir.join("vocab.json");
        let merges = tok_dir.join("merges.txt");

        let bpe = BPE::from_file(
            vocab.to_str().ok_or_else(|| anyhow!("bad vocab path"))?,
            merges.to_str().ok_or_else(|| anyhow!("bad merges path"))?,
        )
        .build()
        .map_err(|e| anyhow!("BPE build: {}", e))?;

        let mut tokenizer = Tokenizer::new(bpe);

        // Register special tokens so they are never split by BPE
        let special_tokens: Vec<AddedToken> = vec![
            AddedToken::from("<|endoftext|>", true),
            AddedToken::from("<|im_start|>", true),
            AddedToken::from("<|im_end|>", true),
            AddedToken::from("<|audio_pad|>", true),
            AddedToken::from("<|audio_start|>", true),
            AddedToken::from("<|audio_end|>", true),
        ];
        tokenizer.add_special_tokens(&special_tokens);

        Ok(Self {
            conv_frontend,
            encoder,
            decoder,
            tokenizer,
            mel: MelSpectrogram::new(),
        })
    }

    /// Transcribe a complete audio buffer (speech_start → speech_end).
    /// `audio` must be 16kHz mono f32 in [-1, 1].
    pub fn transcribe<F>(&mut self, audio: &[f32], mut on_partial: F) -> Result<String>
    where
        F: FnMut(&str),
    {
        let audio_max = audio.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        log::info!("[STT] Transcribe called with audio len={}, max={:.4}", audio.len(), audio_max);
        log::debug!("[STT] Raw audio first 20: {:?}", &audio[..20.min(audio.len())]);

        let start_time = std::time::Instant::now();
        if audio.len() < FFT_SIZE {
            return Ok(String::new());
        }

        // ── Step 0: Audio Normalization ─────────────────────────────────────
        // ASR models are sensitive to volume. Normalize to peak 0.8 to ensure
        // consistent signal levels.
        let max_val = audio.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        let normalized_audio: Vec<f32> = if max_val > 0.01 {
            let scale = 0.8 / max_val;
            audio.iter().map(|&x| x * scale).collect()
        } else {
            audio.to_vec()
        };

        // ── Step 1: Mel spectrogram ──────────────────────────────────────────
        let mel_start = std::time::Instant::now();
        let mel_frames = self.mel.extract(&normalized_audio);
        let n_frames = mel_frames.nrows();
        log::debug!("[STT] Mel extraction took: {:?} ({} frames)", mel_start.elapsed(), n_frames);
        
        if n_frames == 0 {
            return Ok(String::new());
        }

        let mel_input = mel_frames
            .insert_axis(Axis(0))
            .into_owned(); // [1, n_frames, 128]

        let mel_mean = mel_input.mean().unwrap_or(0.0);
        let mel_std = mel_input.std(0.0);
        let mel_max = mel_input.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        log::info!("[STT] Mel Input: mean={:.4}, std={:.4}, max={:.4}, shape={:?}", 
            mel_mean, mel_std, mel_max, mel_input.shape());

        let (conv_shape, conv_data, a_len) = {
            let conv_out = self.conv_frontend.run(ort::inputs![
                "input_features" => ort::value::Value::from_array(mel_input)?
            ])?;
            let (shape, data) = conv_out["conv_output"]
                .try_extract_tensor::<f32>()
                .map_err(|e| anyhow!("conv_output extract: {:?}", e))?;
            
            let a_len = calculate_audio_token_len(n_frames);
            (shape.to_vec(), data.to_vec(), a_len)
        };

        log::info!("[STT] Calculated audio token length: {} (conv frames: {})", a_len, conv_shape[1]);

        let conv_arr = Array2::from_shape_vec(
            (conv_shape[1] as usize, conv_shape[2] as usize),
            conv_data,
        )?
        .insert_axis(Axis(0))
        .into_owned(); // [1, T_conv, 1536]

        // ── Step 3: Encoder ──────────────────────────────────────────────────
        // Create mask for the encoder: 1 for valid audio tokens, 0 for padding (if any)
        // Since we have a_len active tokens from the conv frontend:
        let mut attn_mask_enc = Array2::<bool>::from_elem((1, conv_shape[1] as usize), false);
        for i in 0..a_len {
            attn_mask_enc[[0, i]] = true;
        }

        let (audio_feat_dim, audio_features_owned) = {
            let enc_out = self.encoder.run(ort::inputs![
                "input_features" => ort::value::Value::from_array(conv_arr)?,
                "feature_attention_mask" => ort::value::Value::from_array(attn_mask_enc)?
            ])?;
            let (enc_shape, enc_data) = enc_out["audio_features"]
                .try_extract_tensor::<f32>()
                .map_err(|e| anyhow!("audio_features extract: {:?}", e))?;
            (enc_shape[2] as usize, enc_data.to_vec())
        };

        // Slice the audio features to the ACTUAL audio length (a_len)
        let audio_features_full = Array2::from_shape_vec(
            (conv_shape[1] as usize, audio_feat_dim),
            audio_features_owned,
        )?;
        let audio_features_arr = audio_features_full
            .slice(s![0..a_len, ..])
            .to_owned()
            .insert_axis(Axis(0))
            .into_owned(); // [1, a_len, 3584]

        log::debug!("[STT] Audio Feat first 10: {:?}", &audio_features_arr.as_slice().unwrap()[..10.min(audio_features_arr.len())]);

        // ── Step 3: Statistics Logging ──────────────────────────────────────
        let feat_mean = audio_features_arr.mean().unwrap_or(0.0);
        let feat_std = audio_features_arr.std(0.0);
        let feat_max = audio_features_arr.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        log::info!("[STT] Audio Features: mean={:.4}, std={:.4}, max={:.4}, shape={:?}", 
            feat_mean, feat_std, feat_max, audio_features_arr.shape());

        // ── Step 4: Build Prompt ─────────────────────────────────────────────
        // We build the prompt dynamically based on a_len.
        // Format: <|im_start|>user\n<|audio_start|><|audio_pad|...a_len...|><|audio_end|>\n<|im_end|>\n<|im_start|>assistant\n<asr_text>
        
        let mut input_ids: Vec<i64> = Vec::new();
        
        // 1. <|im_start|>user\n
        input_ids.push(IM_START_ID);
        let user_header = self.tokenizer.encode("user\n", false).map_err(|e| anyhow!(e))?;
        input_ids.extend(user_header.get_ids().iter().map(|&x| x as i64));
        
        // 2. <|audio_start|><|audio_pad|...a_len...|><|audio_end|>
        input_ids.push(AUDIO_START_ID);
        input_ids.extend(std::iter::repeat(AUDIO_PAD_ID).take(a_len));
        input_ids.push(AUDIO_END_ID);
        
        // 3. \n<|im_end|>\n
        let user_footer = self.tokenizer.encode("\n", false).map_err(|e| anyhow!(e))?;
        input_ids.extend(user_footer.get_ids().iter().map(|&x| x as i64));
        input_ids.push(IM_END_ID);
        
        // 4. \n<|im_start|>assistant\n<asr_text>
        input_ids.push(IM_START_ID);
        let asst_header = self.tokenizer.encode("assistant\n", false).map_err(|e| anyhow!(e))?;
        input_ids.extend(asst_header.get_ids().iter().map(|&x| x as i64));
        input_ids.push(ASR_TEXT_ID);

        let s0 = input_ids.len();
        log::info!("[STT] Prompt constructed with {} tokens ({} audio pads)", s0, a_len);

        if s0 + MAX_NEW_TOKENS > MAX_TOTAL_LEN {
            return Err(anyhow!(
                "Prompt ({} tokens) + max_new_tokens ({}) exceeds MAX_TOTAL_LEN ({})",
                s0, MAX_NEW_TOKENS, MAX_TOTAL_LEN
            ));
        }

        // ── Step 5: Allocate KV caches ───────────────────────────────────────
        let mut kv_caches: Vec<Array4<f32>> = (0..NUM_LAYERS * 2)
            .map(|_| Array4::zeros((1, MAX_TOTAL_LEN, NUM_KV_HEADS, HEAD_DIM)))
            .collect();

        // ── Step 6: Decode ───────────────────────────────────────────────────
        let mut generated_tokens: Vec<u32> = Vec::new();
        let mut cur_len: usize = 0;

        let prefill_start = std::time::Instant::now();
        let logits_data = self.run_decoder_step_inner(
            &input_ids,
            cur_len,
            &audio_features_arr,
            &mut kv_caches,
        )?;
        let prefill_duration = prefill_start.elapsed();
        cur_len += s0;

        let vocab_size = 151936_usize;
        let last_pos_offset = (s0 - 1) * vocab_size;
        let last_logits = &logits_data[last_pos_offset..last_pos_offset + vocab_size];

        let mut indexed_logits: Vec<(usize, f32)> = last_logits.iter().enumerate().map(|(i, &v)| (i, v)).collect();
        indexed_logits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // Log top tokens for debugging
        let top_10: Vec<String> = indexed_logits.iter().take(10).map(|(id, val)| {
            let token_str = self.tokenizer.id_to_token(*id as u32).unwrap_or_else(|| "unk".to_string());
            format!("{} ({:.2})", token_str, val)
        }).collect();
        log::info!("[STT] Prefill Top 10: {}", top_10.join(", "));

        let next_id = indexed_logits[0].0 as i64;

        if next_id != EOS_ID {
            generated_tokens.push(next_id as u32);

            for _ in 0..MAX_NEW_TOKENS - 1 {
                let last_tok = *generated_tokens.last().unwrap() as i64;
                let step_ids = vec![last_tok];

                let step_logits = self.run_decoder_step_inner(
                    &step_ids,
                    cur_len,
                    &audio_features_arr,
                    &mut kv_caches,
                )?;
                cur_len += 1;

                let mut indexed_logits: Vec<(usize, f32)> = step_logits[..vocab_size]
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| (i, v))
                    .collect();
                indexed_logits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                
                let token = indexed_logits[0].0 as i64;
                if token == EOS_ID {
                    break;
                }
                generated_tokens.push(token as u32);
            }
        }

        if !generated_tokens.is_empty() {
            if let Ok(partial_text) = self.tokenizer.decode(&generated_tokens, true) {
                // Byte-Level BPE cleaning: remove artifacts like Ġ and normalize whitespace
                let cleaned = partial_text
                    .replace('\u{0120}', " ")
                    .replace('\u{00A0}', " ")
                    .replace("  ", " ")
                    .trim()
                    .to_string();

                if !cleaned.is_empty() {
                    on_partial(&cleaned);
                }
            }
        }

        // ── Step 7: Decode tokens → string ──────────────────────────────────
        if generated_tokens.is_empty() {
            return Ok(String::new());
        }
        let decode_start = std::time::Instant::now();
        let text = self
            .tokenizer
            .decode(&generated_tokens, true)
            .map_err(|e| anyhow!("token decode: {}", e))?;
        let decode_duration = decode_start.elapsed();

        let cleaned_final = text
            .replace('\u{0120}', " ")
            .replace('\u{00A0}', " ")
            .replace("  ", " ")
            .trim()
            .to_string();

        log::info!(
            "[STT] Transcribe took: {:?} (Prefill: {:?}, Decode: {:?}, Tokens: {})", 
            start_time.elapsed(), prefill_duration, decode_duration, generated_tokens.len()
        );
        log::info!("[STT] Generated IDs: {:?}", generated_tokens);
        Ok(cleaned_final)
    }

    /// Run one decoder step. Updates kv_caches in-place.
    /// Returns logits as a flat Vec<f32> of shape [batch=1, step_len, vocab].
    fn run_decoder_step_inner(
        &mut self,
        input_ids: &[i64],
        cur_len: usize,
        audio_features: &ndarray::Array3<f32>,
        kv_caches: &mut Vec<Array4<f32>>,
    ) -> Result<Vec<f32>> {
        let step_len = input_ids.len();
        let batch = 1_usize;

        // input_ids: [1, step_len]
        let ids_arr = Array2::from_shape_vec(
            (batch, step_len),
            input_ids.iter().map(|&x| x as i64).collect(),
        )?;

        // attention_mask: [1, cur_len + step_len] — must cover the entire sequence (past + current)
        let attn_mask_dec = Array2::<i64>::from_elem((batch, cur_len + step_len), 1);

        // cache_position: [step_len] — positions in the KV cache
        let cache_pos: Array1<i64> = Array1::from_iter(
            (cur_len..cur_len + step_len).map(|x| x as i64),
        );

        // Build feed dynamically (56 cache tensors can't use the inputs![] macro)
        let mut feed: Vec<(Cow<str>, ort::value::Value)> = vec![
            ("input_ids".into(),       ort::value::Value::from_array(ids_arr)?.into()),
            ("audio_features".into(),  ort::value::Value::from_array(audio_features.clone())?.into()),
            ("attention_mask".into(),  ort::value::Value::from_array(attn_mask_dec)?.into()),
            ("cache_position".into(),  ort::value::Value::from_array(cache_pos)?.into()),
        ];

        for i in 0..NUM_LAYERS {
            feed.push((
                format!("cache_key_{}", i).into(),
                ort::value::Value::from_array(kv_caches[2 * i].clone())?.into(),
            ));
            feed.push((
                format!("cache_value_{}", i).into(),
                ort::value::Value::from_array(kv_caches[2 * i + 1].clone())?.into(),
            ));
        }

        let outputs = self.decoder.run(feed)?;

        // Extract logits
        let (logits_shape, logits_raw) = outputs["logits"]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow!("logits extract: {:?}", e))?;
        let logits_data: Vec<f32> = logits_raw.to_vec();

        // Update KV caches from key_delta / value_delta outputs
        for i in 0..NUM_LAYERS {
            let (_kd_shape, kd_raw) = outputs[format!("key_delta_{}", i).as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|e| anyhow!("key_delta_{} extract: {:?}", i, e))?;

            let (_vd_shape, vd_raw) = outputs[format!("value_delta_{}", i).as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|e| anyhow!("value_delta_{} extract: {:?}", i, e))?;

            // key_delta: [1, step_len, NUM_KV_HEADS, HEAD_DIM]
            let kd = ArrayView4::from_shape(
                (1usize, step_len, NUM_KV_HEADS, HEAD_DIM),
                kd_raw,
            )
            .map_err(|e| anyhow!("key_delta_{} reshape: {}", i, e))?;

            let vd = ArrayView4::from_shape(
                (1usize, step_len, NUM_KV_HEADS, HEAD_DIM),
                vd_raw,
            )
            .map_err(|e| anyhow!("value_delta_{} reshape: {}", i, e))?;

            // Write into pre-allocated cache at positions [cur_len .. cur_len+step_len]
            kv_caches[2 * i]
                .slice_mut(s![.., cur_len..cur_len + step_len, .., ..])
                .assign(&kd);
            kv_caches[2 * i + 1]
                .slice_mut(s![.., cur_len..cur_len + step_len, .., ..])
                .assign(&vd);
        }

        let _ = logits_shape; // shape checked implicitly
        Ok(logits_data)
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

