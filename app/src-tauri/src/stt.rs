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
const MAX_TOTAL_LEN: usize = 256;
const MAX_NEW_TOKENS: usize = 64;

// Token IDs (from tokenizer_config.json)
const IM_START_ID: i64 = 151644;
const IM_END_ID: i64 = 151645;
const EOS_ID: i64 = 151645;  // <|im_end|> is EOS for Qwen3-ASR
const AUDIO_PAD_ID: i64 = 151676;

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

        // Slaney normalization
        for mut row in filters.axis_iter_mut(Axis(0)) {
            let s = row.sum();
            if s > 0.0 {
                row.mapv_inplace(|x| x / s);
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
                out[[i, j]] = mel_row[j].max(1e-10).log10();
            }
        }
        out
    }
}

// ─── Commands ─────────────────────────────────────────────────────────────────

pub enum SttCommand {
    Audio(Vec<f32>),
    Clear,
}

// ─── Engine ───────────────────────────────────────────────────────────────────

pub struct SttEngine {
    conv_frontend: Session,
    encoder: Session,
    decoder: Session,
    tokenizer: Tokenizer,
    mel: MelSpectrogram,
    /// Pre-encoded prompt prefix token IDs (before audio pads)
    prompt_prefix: Vec<i64>,
    /// Pre-encoded prompt suffix token IDs (after audio pads)
    prompt_suffix: Vec<i64>,
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

        // ── Pre-encode static prompt parts ──────────────────────────────────
        // Full prompt: <|im_start|>user\nAudio 1:  [audio_pad×N]<|im_end|>\n<|im_start|>assistant\n
        //
        // prefix = [IM_START] + encode("user\nAudio 1:  ")
        // suffix = [IM_END]   + encode("\n") + [IM_START] + encode("assistant\n")

        let prefix_text = tokenizer
            .encode("user\nAudio 1:  ", false)
            .map_err(|e| anyhow!("encode prefix: {}", e))?;
        let newline_text = tokenizer
            .encode("\n", false)
            .map_err(|e| anyhow!("encode newline: {}", e))?;
        let asst_text = tokenizer
            .encode("assistant\n", false)
            .map_err(|e| anyhow!("encode assistant: {}", e))?;

        let mut prompt_prefix = vec![IM_START_ID];
        prompt_prefix.extend(prefix_text.get_ids().iter().map(|&x| x as i64));

        let mut prompt_suffix = vec![IM_END_ID];
        prompt_suffix.extend(newline_text.get_ids().iter().map(|&x| x as i64));
        prompt_suffix.push(IM_START_ID);
        prompt_suffix.extend(asst_text.get_ids().iter().map(|&x| x as i64));

        Ok(Self {
            conv_frontend,
            encoder,
            decoder,
            tokenizer,
            mel: MelSpectrogram::new(),
            prompt_prefix,
            prompt_suffix,
        })
    }

    /// Transcribe a complete audio buffer (speech_start → speech_end).
    /// `audio` must be 16kHz mono f32 in [-1, 1].
    pub fn transcribe(&mut self, audio: &[f32]) -> Result<String> {
        if audio.len() < FFT_SIZE {
            return Ok(String::new());
        }

        // ── Step 1: Mel spectrogram ──────────────────────────────────────────
        // mel_frames: [n_frames, 128]
        let mel_frames = self.mel.extract(audio);
        let n_frames = mel_frames.nrows();
        if n_frames == 0 {
            return Ok(String::new());
        }

        // conv_frontend wants [batch, n_frames, 128]
        let mel_input = mel_frames
            .insert_axis(Axis(0))
            .into_owned(); // [1, n_frames, 128]

        // ── Step 2: Conv frontend ────────────────────────────────────────────
        // output: [1, n_audio_tokens, 896] (dynamic n_audio_tokens)
        let (n_audio_tokens, conv_dim, conv_data_owned): (usize, usize, Vec<f32>) = {
            let conv_out = self.conv_frontend.run(ort::inputs![
                "input_features" => ort::value::Value::from_array(mel_input)?
            ])?;
            let (conv_shape, conv_data) = conv_out["conv_output"]
                .try_extract_tensor::<f32>()
                .map_err(|e| anyhow!("conv_output extract: {:?}", e))?;
            (conv_shape[1] as usize, conv_shape[2] as usize, conv_data.to_vec())
        }; // conv_out dropped here — borrow on self.conv_frontend released

        // Reshape to [1, n_audio_tokens, conv_dim]
        let conv_arr = Array2::from_shape_vec(
            (n_audio_tokens, conv_dim),
            conv_data_owned,
        )?
        .insert_axis(Axis(0))
        .into_owned(); // [1, n_audio_tokens, conv_dim]

        // ── Step 3: Encoder ──────────────────────────────────────────────────
        // input_features: [1, n_audio_tokens, 896]
        // feature_attention_mask: [1, n_audio_tokens] bool (all ones = valid)
        let attn_mask_enc = Array2::<i64>::ones((1, n_audio_tokens));

        let (audio_feat_dim, audio_features_owned): (usize, Vec<f32>) = {
            let enc_out = self.encoder.run(ort::inputs![
                "input_features" => ort::value::Value::from_array(conv_arr)?,
                "feature_attention_mask" => ort::value::Value::from_array(attn_mask_enc)?
            ])?;
            let (enc_shape, enc_data) = enc_out["audio_features"]
                .try_extract_tensor::<f32>()
                .map_err(|e| anyhow!("audio_features extract: {:?}", e))?;
            (enc_shape[2] as usize, enc_data.to_vec())
        }; // enc_out dropped here — borrow on self.encoder released
        let audio_features_arr = Array2::from_shape_vec(
            (n_audio_tokens, audio_feat_dim),
            audio_features_owned,
        )?
        .insert_axis(Axis(0))
        .into_owned(); // [1, n_audio_tokens, 1024]

        // ── Step 4: Build prompt input_ids ───────────────────────────────────
        // <|im_start|>user\nAudio 1:  [audio_pad×N]<|im_end|>\n<|im_start|>assistant\n
        let mut input_ids: Vec<i64> = Vec::new();
        input_ids.extend_from_slice(&self.prompt_prefix);
        input_ids.extend(std::iter::repeat(AUDIO_PAD_ID).take(n_audio_tokens));
        input_ids.extend_from_slice(&self.prompt_suffix);

        let s0 = input_ids.len();

        // Check if prompt fits in KV cache
        if s0 + MAX_NEW_TOKENS > MAX_TOTAL_LEN {
            return Err(anyhow!(
                "Prompt ({} tokens) + max_new_tokens ({}) exceeds MAX_TOTAL_LEN ({})",
                s0, MAX_NEW_TOKENS, MAX_TOTAL_LEN
            ));
        }

        // ── Step 5: Allocate KV caches ───────────────────────────────────────
        // 28 layers × 2 (key + value) = 56 tensors, each [1, 256, 8, 128]
        let mut kv_caches: Vec<Array4<f32>> = (0..NUM_LAYERS * 2)
            .map(|_| Array4::zeros((1, MAX_TOTAL_LEN, NUM_KV_HEADS, HEAD_DIM)))
            .collect();

        // ── Step 6: Decode ───────────────────────────────────────────────────
        let mut generated_tokens: Vec<u32> = Vec::new();
        let mut cur_len: usize = 0;

        // First pass: prefill with full prompt
        let logits_data = self.run_decoder_step_inner(
            &input_ids,
            cur_len,
            &audio_features_arr,
            &mut kv_caches,
        )?;
        cur_len += s0;

        // Greedy token from last position
        let vocab_size = 151936_usize;
        let last_pos_offset = (s0 - 1) * vocab_size;
        let next_id = argmax(&logits_data[last_pos_offset..last_pos_offset + vocab_size]) as i64;

        if next_id == EOS_ID {
            // Empty response — return empty
        } else {
            generated_tokens.push(next_id as u32);

            // Decode loop: one token at a time
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

                let token = argmax(&step_logits[..vocab_size]) as i64;
                if token == EOS_ID {
                    break;
                }
                generated_tokens.push(token as u32);
            }
        }

        // ── Step 7: Decode tokens → string ──────────────────────────────────
        if generated_tokens.is_empty() {
            return Ok(String::new());
        }

        let text = self
            .tokenizer
            .decode(&generated_tokens, true)
            .map_err(|e| anyhow!("token decode: {}", e))?;

        Ok(text.trim().to_string())
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

        // attention_mask: [1, step_len] — all ones (every token is attended to)
        let attn_mask_dec = Array2::<i64>::ones((batch, step_len));

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

fn argmax(slice: &[f32]) -> usize {
    slice
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Less))
        .map(|(i, _)| i)
        .unwrap_or(0)
}
