use anyhow::Result;
use ndarray::{Array1, Array2, Array3};
use ort::session::Session;
use std::sync::Arc;
use tokio::sync::mpsc;
use serde_json::json;
use rustfft::{FftPlanner, num_complex::Complex, Fft};

pub struct VadEngine {
    session: Session,
    h: [Array2<f32>; 4], // 4 hidden states for TEN VAD
    feature_buffer: Vec<Array1<f32>>,
    hann_window: Array1<f32>,
    mel_filterbank: Vec<Vec<(usize, f32)>>, // (index, weight) for sparse mel filter
    means: Array1<f32>,
    stds: Array1<f32>,
    fft: Arc<dyn Fft<f32>>,
}

impl VadEngine {
    pub fn new(model_path: &str) -> Result<Self> {
        let session = Session::builder()?
            .with_intra_threads(1)
            .map_err(|e| anyhow::anyhow!("Failed to set intra threads: {:?}", e))?
            .commit_from_file(model_path)
            .map_err(|e| anyhow::anyhow!("Failed to load VAD model: {:?}", e))?;

        for input in session.inputs() {
            log::info!("[VAD] Model Input: {}", input.name());
        }
        for output in session.outputs() {
            log::info!("[VAD] Model Output: {}", output.name());
        }

        let h = [
            Array2::zeros((1, 64)),
            Array2::zeros((1, 64)),
            Array2::zeros((1, 64)),
            Array2::zeros((1, 64)),
        ];

        let hann_window = Array1::from_shape_fn(768, |i| {
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / 767.0).cos())
        });

        // FIX 3A: Use 40 mel bins (matches normalization stats from coeff.h).
        // The old code used 40-bin filterbank but created a 41-element feature
        // vector with a 0.0 placeholder at [40]. That 0.0 got normalized to -0.80
        // by the outlier stats (mean=92, std=115), injecting a constant strong
        // signal into every frame and creating a ~0.50 probability floor on silence.
        let mel_filterbank = Self::create_mel_filterbank(768, 16000.0, 40);
        let (means, stds) = Self::get_normalization_stats();

        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(768);

        Ok(Self {
            session,
            h,
            feature_buffer: Vec::new(),
            hann_window,
            mel_filterbank,
            means,
            stds,
            fft,
        })
    }

    fn create_mel_filterbank(fft_size: usize, sample_rate: f32, num_bins: usize) -> Vec<Vec<(usize, f32)>> {
        let low_mel = 0.0;
        let high_mel = 2595.0 * (1.0 + (sample_rate / 2.0) / 700.0).log10();
        let mel_points: Vec<f32> = (0..num_bins + 2)
            .map(|i| low_mel + i as f32 * (high_mel - low_mel) / (num_bins + 1) as f32)
            .collect();
        
        let hz_points: Vec<f32> = mel_points.iter()
            .map(|&m| 700.0 * (10.0f32.powf(m / 2595.0) - 1.0))
            .collect();
        
        let bins: Vec<usize> = hz_points.iter()
            .map(|&hz| (hz * (fft_size as f32 + 1.0) / sample_rate) as usize)
            .collect();

        let mut filterbank = vec![Vec::new(); num_bins];
        let num_fft_bins = (fft_size / 2) + 1;

        for i in 0..num_bins {
            for j in bins[i]..bins[i + 1] {
                let weight = (j - bins[i]) as f32 / (bins[i + 1] - bins[i]) as f32;
                if j < num_fft_bins {
                    filterbank[i].push((j, weight));
                }
            }
            for j in bins[i + 1]..bins[i + 2] {
                let weight = (bins[i + 2] - j) as f32 / (bins[i + 2] - bins[i + 1]) as f32;
                if j < num_fft_bins {
                    filterbank[i].push((j, weight));
                }
            }
        }
        filterbank
    }

    fn get_normalization_stats() -> (Array1<f32>, Array1<f32>) {
        // 40-bin stats from coeff.h (TEN VAD reference implementation).
        // These 40 values match the 40 mel bins exactly — no placeholder needed.
        let means = Array1::from_vec(vec![
            -8.198236465454e+00, -6.265716552734e+00, -5.483818531036e+00,
            -4.758691310883e+00, -4.417088985443e+00, -4.142892837524e+00,
            -3.912850379944e+00, -3.845927953720e+00, -3.657090425491e+00,
            -3.723418712616e+00, -3.876134157181e+00, -3.843890905380e+00,
            -3.690405130386e+00, -3.756065845490e+00, -3.698696136475e+00,
            -3.650463104248e+00, -3.700468778610e+00, -3.567321300507e+00,
            -3.498900175095e+00, -3.477807044983e+00, -3.458816051483e+00,
            -3.444923877716e+00, -3.401328563690e+00, -3.306261301041e+00,
            -3.278556823730e+00, -3.233250856400e+00, -3.198616027832e+00,
            -3.204526424408e+00, -3.208798646927e+00, -3.257838010788e+00,
            -3.381376743317e+00, -3.534021377563e+00, -3.640867948532e+00,
            -3.726858854294e+00, -3.773730993271e+00, -3.804667234421e+00,
            -3.832901000977e+00, -3.871120452881e+00, -3.990592956543e+00,
            -4.480289459229e+00, 0.000000000000e+00, // 41st bin: Neutral mean
        ]);
        let stds = Array1::from_vec(vec![
            5.166063785553e+00, 4.977209568024e+00, 4.698895931244e+00,
            4.630621433258e+00, 4.634347915649e+00, 4.641156196594e+00,
            4.640676498413e+00, 4.666367053986e+00, 4.650534629822e+00,
            4.640020847321e+00, 4.637400150299e+00, 4.620099067688e+00,
            4.596316337585e+00, 4.562654972076e+00, 4.554360389709e+00,
            4.566910743713e+00, 4.562489986420e+00, 4.562412738800e+00,
            4.585299491882e+00, 4.600179672241e+00, 4.592845916748e+00,
            4.585922718048e+00, 4.583496570587e+00, 4.626092910767e+00,
            4.626957893372e+00, 4.626289367676e+00, 4.637005805969e+00,
            4.683015823364e+00, 4.726813793182e+00, 4.734289646149e+00,
            4.753227233887e+00, 4.849722862244e+00, 4.869434833527e+00,
            4.884482860565e+00, 4.921327114105e+00, 4.959212303162e+00,
            4.996619224548e+00, 5.044823646545e+00, 5.072216987610e+00,
            4.096439361572e+00, 1.000000000000e+00, // 41st bin: Neutral std
        ]);
        (means, stds)
    }

    pub async fn run_loop<C>(
        &mut self,
        mut consumer: C,
        tx: mpsc::Sender<serde_json::Value>,
        stt_tx: mpsc::Sender<crate::stt::SttCommand>,
    ) -> Result<()> 
    where 
        C: ringbuf::traits::Consumer<Item = f32> 
    {
        let mut audio_buffer = Vec::with_capacity(768);
        let mut in_speech = false;
        let mut silence_frames = 0;
        // FIX 3B: Consecutive-frame guard — more sensitive now
        let mut speech_confirm_frames: u32 = 0;
        let mut utterance_buffer: Vec<f32> = Vec::new();
        let mut samples_since_partial = 0;
        let mut last_log = std::time::Instant::now();

        let mut current_session_id: u32 = 0;

        loop {
            // Wait for enough samples for a hop (160 samples = 10ms)
            if consumer.occupied_len() >= 160 {
                let mut chunk = vec![0.0f32; 160];
                consumer.pop_slice(&mut chunk);

                // Add to rolling 768-sample context window for VAD inference
                audio_buffer.extend_from_slice(&chunk);
                if audio_buffer.len() > 768 {
                    audio_buffer.drain(0..(audio_buffer.len() - 768));
                }

                if audio_buffer.len() == 768 {
                    // Extract features
                    let feat = self.extract_features(&audio_buffer);
                    self.feature_buffer.push(feat);

                    if self.feature_buffer.len() >= 3 {
                        if self.feature_buffer.len() > 3 {
                            self.feature_buffer.remove(0);
                        }

                        // Run inference
                        let prob = self.process_inference()?;
                        
                        // Throttled logging for debug
                        if last_log.elapsed() > std::time::Duration::from_secs(1) {
                            log::debug!("[VAD] Prob: {:.4}, in_speech: {}", prob, in_speech);
                            last_log = std::time::Instant::now();
                        }

                        // VAD State Machine - Threshold: 0.6, Confirm: 5 frames (50ms)
                        if prob > 0.60 {
                            speech_confirm_frames += 1;
                            silence_frames = 0;
                            
                            if !in_speech && speech_confirm_frames >= 5 {
                                in_speech = true;
                                current_session_id += 1;
                                log::info!("[VAD] >>> SPEECH DETECTED (prob: {:.4}, session: {})", prob, current_session_id);
                                let _ = tx.send(json!({ 
                                    "type": "speech_start", 
                                    "session_id": current_session_id 
                                })).await;
                                utterance_buffer.clear();
                                samples_since_partial = 0;
                            }

                            if in_speech {
                                utterance_buffer.extend_from_slice(&chunk);
                                samples_since_partial += chunk.len();

                                // Every 800ms (12800 samples at 16kHz), send a partial transcript command
                                if samples_since_partial >= 12800 {
                                    log::debug!("[VAD] Triggering partial transcription (session: {})", current_session_id);
                                    if let Err(e) = stt_tx.try_send(crate::stt::SttCommand::Partial(current_session_id, utterance_buffer.clone())) {
                                        log::warn!("[VAD] STT queue full, skipping partial update: {:?}", e);
                                    }
                                    samples_since_partial = 0;
                                }
                            }
                        } else {
                            speech_confirm_frames = 0;

                            if in_speech {
                                utterance_buffer.extend_from_slice(&chunk);
                                samples_since_partial += chunk.len();

                                // Still check for partials even during low-prob frames if we are in_speech
                                if samples_since_partial >= 12800 {
                                    log::debug!("[VAD] Triggering partial transcription (session: {})", current_session_id);
                                    if let Err(e) = stt_tx.try_send(crate::stt::SttCommand::Partial(current_session_id, utterance_buffer.clone())) {
                                        log::warn!("[VAD] STT queue full, skipping partial update: {:?}", e);
                                    }
                                    samples_since_partial = 0;
                                }

                                silence_frames += 1;

                                if silence_frames > 100 { // 1.0s timeout
                                    in_speech = false;
                                    log::info!("[VAD] <<< SPEECH ENDED (silence timeout, session: {})", current_session_id);
                                    let _ = tx.send(json!({ 
                                        "type": "speech_end",
                                        "session_id": current_session_id 
                                    })).await;
                                    
                                    if utterance_buffer.len() >= 6400 { // 0.4s min
                                        log::info!("[VAD] Sending final {} samples to STT (session: {})", utterance_buffer.len(), current_session_id);
                                        // CRITICAL: We MUST NOT drop final updates.
                                        // Spawn a task to send it so we don't block the VAD loop, but use .send().await to ensure it gets in the queue.
                                        let stt_tx_clone = stt_tx.clone();
                                        let sid = current_session_id;
                                        let buf = utterance_buffer.clone();
                                        tokio::spawn(async move {
                                            if let Err(e) = stt_tx_clone.send(crate::stt::SttCommand::Final(sid, buf)).await {
                                                log::error!("[VAD] CRITICAL: Failed to queue final STT command: {:?}", e);
                                            }
                                        });
                                    }
                                    utterance_buffer.clear();
                                    samples_since_partial = 0;
                                    self.reset_states();
                                }
                            }
                        }
                    }
                }
            } else {
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            }
        }
    }

    fn extract_features(&self, audio: &[f32]) -> Array1<f32> {
        // 1. Hann window
        let mut fft_buffer: Vec<Complex<f32>> = audio.iter()
            .zip(self.hann_window.iter())
            .map(|(&s, &w)| Complex::new(s * w, 0.0))
            .collect();

        // 2. Magnitude Spectrum
        self.fft.process(&mut fft_buffer);
        let mag: Vec<f32> = fft_buffer.iter().take(385).map(|c: &Complex<f32>| c.norm()).collect();

        // FIX: The model expects 41 features.
        // We use 40 mel bins (standard) and set the 41st to a neutral value (0.0).
        let mut mel_energy = Array1::zeros(41);
        
        for (i, bin_weights) in self.mel_filterbank.iter().enumerate() {
            let mut energy = 0.0f32;
            for &(idx, weight) in bin_weights {
                if idx < mag.len() {
                    energy += mag[idx] * weight;
                }
            }
            mel_energy[i] = energy.max(1e-20).ln();
        }

        // Normalize using 41-bin stats
        for i in 0..41 {
            mel_energy[i] = (mel_energy[i] - self.means[i]) / (self.stds[i] + 1e-20);
        }

        mel_energy
    }

    fn process_inference(&mut self) -> Result<f32> {
        // Stack 3 frames: [1, 3, 41]
        let input_0 = Array3::from_shape_fn((1, 3, 41), |(_, f, i)| self.feature_buffer[f][i]);
        
        // Map tensors to model input names dynamically
        let input_names: Vec<String> = self.session.inputs().iter().map(|i| i.name().to_string()).collect();
        
        let inputs = vec![
            (input_names[0].as_str(), ort::value::Value::from_array(input_0)?),
            (input_names[1].as_str(), ort::value::Value::from_array(self.h[0].clone())?),
            (input_names[2].as_str(), ort::value::Value::from_array(self.h[1].clone())?),
            (input_names[3].as_str(), ort::value::Value::from_array(self.h[2].clone())?),
            (input_names[4].as_str(), ort::value::Value::from_array(self.h[3].clone())?),
        ];

        let outputs = self.session.run(inputs)?;
        
        // Output probability - try to get by name if possible, fallback to index 0
        let prob_val = outputs.get("prob")
            .or_else(|| outputs.get("logits"))
            .or_else(|| outputs.get("output"))
            .unwrap_or(&outputs[0]);

        let prob = match prob_val.try_extract_tensor::<f32>() {
            Ok(view) => view.1[0],
            Err(e) => {
                log::error!("[VAD] Failed to extract probability tensor: {:?}. Output count: {}", e, outputs.len());
                return Err(anyhow::anyhow!("VAD extraction failed: {}", e));
            }
        };

        // Update hidden states
        for i in 0..4 {
            let name = format!("h{}", i + 1);
            let h_val = outputs.get(&name).unwrap_or(&outputs[i + 1]);
            
            match h_val.try_extract_tensor::<f32>() {
                Ok(view) => {
                    self.h[i] = Array2::from_shape_vec((1, 64), view.1.to_vec())?;
                }
                Err(e) => {
                    log::error!("[VAD] Failed to extract hidden state {}: {:?}", name, e);
                }
            }
        }

        Ok(prob)
    }

    fn reset_states(&mut self) {
        for i in 0..4 {
            self.h[i].fill(0.0);
        }
    }

    // ── Test helpers (pub so integration tests in tests/ can access them) ──────

    pub fn extract_features_pub(&self, audio: &[f32]) -> Array1<f32> {
        self.extract_features(audio)
    }

    pub fn process_frame_pub(&mut self, audio: &[f32]) -> Result<f32> {
        let feat = self.extract_features(audio);
        self.feature_buffer.push(feat);
        if self.feature_buffer.len() > 3 {
            self.feature_buffer.remove(0);
        }
        if self.feature_buffer.len() == 3 {
            self.process_inference()
        } else {
            Ok(0.0)
        }
    }
}
