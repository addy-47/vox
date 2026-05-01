use anyhow::Result;
use ndarray::{Array1, Array2, Axis, s};
use ort::session::Session;
use ort::inputs;
use rustfft::{FftPlanner, num_complex::Complex};
use std::f32::consts::PI;
use std::path::Path;
use tokenizers::Tokenizer;
use tokenizers::models::bpe::BPE;

pub struct MelSpectrogram {
    fft_size: usize,
    hop_length: usize,
    num_mels: usize,
    mel_filters: Array2<f32>,
    window: Array1<f32>,
}

impl MelSpectrogram {
    pub fn new(fft_size: usize, hop_length: usize, num_mels: usize, sample_rate: f32) -> Self {
        let window = Array1::from_shape_fn(fft_size, |i| {
            0.5 * (1.0 - (2.0 * PI * i as f32 / (fft_size as f32)).cos())
        });

        let mel_filters = Self::create_mel_filters(fft_size, sample_rate, num_mels);

        Self {
            fft_size,
            hop_length,
            num_mels,
            mel_filters,
            window,
        }
    }

    fn hz_to_mel(hz: f32) -> f32 {
        2595.0 * (1.0 + hz / 700.0).log10()
    }

    fn mel_to_hz(mel: f32) -> f32 {
        700.0 * (10.0f32.powf(mel / 2595.0) - 1.0)
    }

    fn create_mel_filters(fft_size: usize, sample_rate: f32, num_mels: usize) -> Array2<f32> {
        let f_min = 0.0;
        let f_max = sample_rate / 2.0;
        let mel_min = Self::hz_to_mel(f_min);
        let mel_max = Self::hz_to_mel(f_max);

        let mut mel_points = Array1::linspace(mel_min, mel_max, num_mels + 2);
        let hz_points = mel_points.mapv(Self::mel_to_hz);

        let bin_points = hz_points.mapv(|hz| (fft_size as f32 + 1.0) * hz / sample_rate);
        
        let mut filters = Array2::zeros((num_mels, fft_size / 2 + 1));

        for i in 0..num_mels {
            let left = bin_points[i] as usize;
            let center = bin_points[i + 1] as usize;
            let right = bin_points[i + 2] as usize;

            for j in left..center {
                filters[[i, j]] = (j - left) as f32 / (center - left) as f32;
            }
            for j in center..right {
                if j < fft_size / 2 + 1 {
                    filters[[i, j]] = (right - j) as f32 / (right - center) as f32;
                }
            }
        }

        // Slaney normalization
        for mut row in filters.axis_iter_mut(Axis(0)) {
            let sum = row.sum();
            if sum > 0.0 {
                row.mapv_inplace(|x| x / sum);
            }
        }

        filters
    }

    pub fn extract(&self, audio: &[f32]) -> Array2<f32> {
        let num_frames = (audio.len() - self.fft_size) / self.hop_length + 1;
        let mut mel_spectrogram = Array2::zeros((num_frames, self.num_mels));
        
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(self.fft_size);

        for i in 0..num_frames {
            let start = i * self.hop_length;
            let end = start + self.fft_size;
            let frame = &audio[start..end];
            
            let mut input: Vec<Complex<f32>> = frame.iter()
                .zip(self.window.iter())
                .map(|(s, w)| Complex::new(s * w, 0.0))
                .collect();

            fft.process(&mut input);

            let mut power_spec = Array1::zeros(self.fft_size / 2 + 1);
            for j in 0..(self.fft_size / 2 + 1) {
                power_spec[j] = input[j].norm_sqr();
            }

            let mel_frame = self.mel_filters.dot(&power_spec);
            for j in 0..self.num_mels {
                mel_spectrogram[[i, j]] = (mel_frame[j].max(1e-10)).log10();
            }
        }

        mel_spectrogram
    }
}

pub enum SttCommand {
    Audio(Vec<f32>),
    Clear,
}

pub struct SttEngine {
    conv_frontend: Session,
    encoder: Session,
    decoder: Session,
    tokenizer: Tokenizer,
    mel_extractor: MelSpectrogram,
    audio_buffer: Vec<f32>,
}

impl SttEngine {
    pub fn new(model_dir: &Path) -> Result<Self> {
        let conv_frontend = Session::builder()?
            .commit_from_file(model_dir.join("conv_frontend.onnx"))?;
        let encoder = Session::builder()?
            .commit_from_file(model_dir.join("encoder.int8.onnx"))?;
        let decoder = Session::builder()?
            .commit_from_file(model_dir.join("decoder.int8.onnx"))?;

        let tokenizer_dir = model_dir.join("tokenizer");
        let vocab_path = tokenizer_dir.join("vocab.json");
        let merges_path = tokenizer_dir.join("merges.txt");

        let bpe = BPE::from_file(
            vocab_path.to_str().ok_or_else(|| anyhow::anyhow!("invalid vocab path"))?,
            merges_path.to_str().ok_or_else(|| anyhow::anyhow!("invalid merges path"))?,
        )
        .build()
        .map_err(|e| anyhow::anyhow!("failed to load BPE: {}", e))?;

        let tokenizer = Tokenizer::new(bpe);

        let mel_extractor = MelSpectrogram::new(400, 160, 128, 16000.0);

        Ok(Self {
            conv_frontend,
            encoder,
            decoder,
            tokenizer,
            mel_extractor,
            audio_buffer: Vec::new(),
        })
    }

    pub fn transcribe(&mut self, new_audio: &[f32]) -> Result<String> {
        self.audio_buffer.extend_from_slice(new_audio);
        
        // Sliding window of 240ms (3840 samples at 16kHz)
        if self.audio_buffer.len() > 3840 {
            self.audio_buffer.drain(0..(self.audio_buffer.len() - 3840));
        }

        if self.audio_buffer.len() < 3840 {
            return Ok("".to_string());
        }

        // 1. Extract Mel Features
        let mel = self.mel_extractor.extract(&self.audio_buffer);
        
        // Convert to [1, 128, T] for conv_frontend
        let mel_input = mel.reversed_axes()
            .insert_axis(Axis(0))
            .into_owned();

        // 2. Conv Frontend
        let outputs = self.conv_frontend.run(inputs![
            "input_features" => ort::value::Value::from_array(mel_input.view())?
        ])?;
        let x = outputs.get("output")
            .ok_or_else(|| anyhow::anyhow!("missing output 'output'"))?;

        // 3. Encoder
        let outputs = self.encoder.run(inputs![
            "inputs" => x
        ])?;
        let enc_out = outputs.get("output")
            .ok_or_else(|| anyhow::anyhow!("missing output 'output'"))?;

        // 4. Greedy Search Decoder
        let mut tokens = vec![];
        
        let mut input_ids = vec![151644i64, 151645i64, 151646i64];
        
        for _ in 0..10 {
            let input_ids_array = Array2::from_shape_vec((1, input_ids.len()), input_ids.clone())?;
            
            let outputs = self.decoder.run(inputs![
                "input_ids" => ort::value::Value::from_array(input_ids_array.view())?,
                "encoder_hidden_states" => enc_out
            ])?;
            
            let logits = outputs.get("logits")
                .ok_or_else(|| anyhow::anyhow!("missing output 'logits'"))?
                .try_extract_tensor::<f32>()?;
            
            let logits_view = logits.view();
            let last_token_logits = logits_view.slice(s![0, -1, ..]);
            
            let mut max_val = f32::MIN;
            let mut max_idx = 0;
            for (idx, &val) in last_token_logits.iter().enumerate() {
                if val > max_val {
                    max_val = val;
                    max_idx = idx;
                }
            }
            
            if max_idx == 151643 {
                break;
            }
            
            tokens.push(max_idx as u32);
            input_ids.push(max_idx as i64);
        }

        let decoded = self.tokenizer.decode(&tokens, true)
            .map_err(|e| anyhow::anyhow!("failed to decode tokens: {}", e))?;

        Ok(decoded)
    }

    pub fn clear_buffer(&mut self) {
        self.audio_buffer.clear();
    }
}
