# Vox v0.8.0 Formal Production Benchmark Results

- **Date:** 2026-06-06 14:17:10
- **OS Platform:** Linux
- **LLM Model:** Llama-3.2-1B-Instruct-Q6_K.gguf
- **STT Model:** Qwen3-ASR
- **TTS Model:** Kokoro-82M + Priyamvada-Medium

## ⚡ Executive Performance Summary

| Metric | Average Benchmark Value | Target Baseline | Status |
| :--- | :--- | :--- | :--- |
| **STT RTF (Real-Time Factor)** | `4.58x` | `< 1.50x (rolling window)` | **Passed** ✅ |
| **LLM Generation Speed** | `8.38 TPS` | `> 1.00 TPS` | **Passed** ✅ |
| **TTFA (Time to First Audio)** | `2.09s` | `< 4.00s` | **Passed** ✅ |
| **Total Turn Latency** | `4.98s` | `< 10.00s` | **Passed** ✅ |
| **Peak Process RSS** | `2492 MB` | `< 7500 MB` | **Passed** ✅ |

## 🧠 Memory Footprint Profiles

| Module | Engine | Model | Memory Allocation (RSS) |
| :--- | :--- | :--- | :--- |
| **STT** | `sherpa-onnx` | `Qwen3-ASR` | `1177 MB` |
| **LLM** | `llama-cpp-2` | `Llama-3.2-1B (Q6_K)` | `977 MB` |
| **TTS** | `kokoro + piper` | `Kokoro-82M + Priyamvada-Medium` | `338 MB` |
| **Total Peak Footprint** | **All Active Workers** | **Full Context Pipeline** | **`2492 MB`** |

## 📋 Granular Run Metrics (10-File Sequence)

| Run | Input File | Audio Dur (s) | Ground Truth | STT Transcript | STT RTF | LLM TPS | TTFA (s) | Total (s) | Peak RSS (MB) |
| :--- | :--- | :---: | :--- | :--- | :---: | :---: | :---: | :---: | :---: |
| #1 | `hiacc_adult_test_AD30129.wav` | 1.2s | जब वो गुफा तक पहुँचे | गुफा तक पहुंचे | 3.20x | 10.41 | 1.53s | 8.35s | 2501 |
| #2 | `hiacc_adult_test_AD21007.wav` | 8.4s | better की hometown पर रहो और बाकी feel करो तो family वगैरह के साथ थोड़ा अपने मन का खाना बना देती है mummy तो वो better रहता है | की होमटॉन पर और बाकी feel करो तो की फील करो तो फैमिली वगैरह के साथ तोड़ा अपने मान का खाना बना देती हूँ मम्मी तो वो better रहता है | 6.92x | 8.33 | 3.60s | 8.28s | 2497 |
| #3 | `hiacc_adult_test_AD25039.wav` | 0.8s | is south | साउथ | 1.82x | 6.76 | 1.30s | 1.33s | 2483 |
| #4 | `hiacc_adult_test_AD25099.wav` | 2.1s | मुझे लगता है वो अपने घर को बहुत miss करता है | क्या तुम्हें अपने घर को बहुत मिस करता है | 7.01x | 9.88 | 1.89s | 5.16s | 2491 |
| #5 | `hiacc_adult_test_AD40103.wav` | 6.0s | चिंटू और राजू ने guardian के दिए गए task complete कर लिए और हर एक task के बाद | जो राजू ने guardian के दिए गए task complete के लिए और हर एक task के बाद. | 5.59x | 5.53 | 2.11s | 2.35s | 2490 |
| #6 | `hiacc_adult_test_AD28031.wav` | 3.6s | food के बारे में तो basically मैं बिरयानी पसंद करता हूं | उनके बारे में तो मैं बेसिकली मैं ब्रिया तो मैं basically मैं ब्रियानी पसंद करता हूँ | 6.85x | 8.86 | 2.61s | 7.90s | 2484 |
| #7 | `hiacc_adult_test_AD59158.wav` | 7.9s | the picture is presenting quite a scenic view of the whole rocky mountains and the vegetation | Presenting quite a scenic view of of the the whole look. | 1.98x | 5.29 | 2.18s | 4.35s | 2506 |
| #8 | `hiacc_adult_test_AD60135.wav` | 3.9s | coming across all and going across all the thoughts | going Across all and going across all the thought. | 3.42x | 9.92 | 1.58s | 3.73s | 2482 |
| #9 | `hiacc_adult_test_AD26125.wav` | 2.7s | उनका एक पुराने बुधिमान विद्वान ने स्वागत किया। | एक पुराने बुधिमान विद्वान ने स्वागत किया | 6.13x | 9.73 | 1.93s | 6.37s | 2493 |
| #10 | `hiacc_adult_test_AD25093.wav` | 1.5s | The wizard will welcomed them | Third, will welcome them. | 2.86x | 9.12 | 2.13s | 1.97s | 2494 |
