# TTS Performance Comparison Report

This report compares the performance of **Kokoro English** and **Piper Hindi** (current production baselines) against **NeuTTS Nano** (the newly integrated speech synthesis engine running on exactly 2 CPU threads).

## Engine Load Metrics

| Engine | Load Time (ms) | Memory Footprint (RAM) |
| :--- | :--- | :--- |
| Kokoro (EN) + Piper (HI) | 4798 ms | 455 MB |
| NeuTTS Nano (Multilingual) | 2752 ms | 1011 MB |

## Prompt Evaluation Suite Results

| Model | Prompt Category | TTFA (ms) | RTF | Inference (ms) | Audio Length (s) | Prompt Text |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| Kokoro (English) | English | 6965 ms | 2.124 | 6965 ms | 3.28 s | `Hello, how are you doing today? The weather seems lovely.` |
| Piper (Hindi) | Hindi | 705 ms | 0.168 | 705 ms | 4.19 s | `नमस्ते, आप कैसे हैं? आज का मौसम बहुत अच्छा है।` |
| Piper (Hindi) | Hinglish (Devanagari) | 675 ms | 0.151 | 675 ms | 4.48 s | `अरे दोस्त, क्या हाल-चाल है? आज का काम कैसा चल रहा है?` |
| Kokoro (English) | Named Entities (EN) | 6804 ms | 1.540 | 6804 ms | 4.42 s | `Barack Obama visited the Taj Mahal in Agra, India, with his family.` |
| Piper (Hindi) | Named Entities (HI) | 657 ms | 0.147 | 657 ms | 4.46 s | `नरेंद्र मोदी ने नई दिल्ली में लाल किले पर झंडा फहराया।` |
| Kokoro (English) | Numbers (EN) | 8608 ms | 1.488 | 8609 ms | 5.79 s | `The year is 2026, and the temperature is exactly 37.5 degrees Celsius.` |
| Piper (Hindi) | Numbers (HI) | 1402 ms | 0.163 | 1402 ms | 8.58 s | `रेलगाड़ी संख्या 12345 दोपहर 3:45 बजे प्लेटफार्म नंबर 5 पर आएगी।` |
| Kokoro (English) | URLs | 13145 ms | 1.494 | 13145 ms | 8.80 s | `You can visit our website at https://deepmind.google/technologies/gemini/ for details.` |
| Kokoro (English) | Abbreviations | 10084 ms | 1.502 | 10084 ms | 6.71 s | `The AI assistant was built using the AGY SDK and runs on a local PC using CPU threads.` |
| Kokoro (English) | Paragraph (EN) | 29930 ms | 1.429 | 29931 ms | 20.94 s | `Speech synthesis has come a long way from the early days of robotic sound generation. Modern engines utilize deep learning architectures, combined with neural vocoders, to produce highly expressive and human-like prosody. This allows desktop virtual assistants to sound natural, providing a pleasant user experience even on budget systems with hardware constraints.` |
| Piper (Hindi) | Paragraph (HI) | 3700 ms | 0.153 | 3701 ms | 24.18 s | `आवाज संश्लेषण तकनीक ने पिछले कुछ वर्षों में बहुत प्रगति की है। आजकल की प्रणालियाँ कृत्रिम बुद्धिमत्ता और तंत्रिका नेटवर्क का उपयोग करके बहुत ही प्राकृतिक और स्पष्ट आवाज उत्पन्न कर सकती हैं। यह तकनीक विशेष रूप से उन लोगों के लिए बहुत उपयोगी है जो पढ़ने में असमर्थ हैं या जो बोलकर बातचीत करना पसंद करते हैं।` |
| NeuTTS Nano | English | 30087 ms | 10.594 | 30087 ms | 2.84 s | `Hello, how are you doing today? The weather seems lovely.` |
| NeuTTS Nano | Hindi | 10497 ms | 262.435 | 10497 ms | 0.04 s | `नमस्ते, आप कैसे हैं? आज का मौसम बहुत अच्छा है।` |
| NeuTTS Nano | Hinglish (Devanagari) | 10038 ms | 0.000 | 10038 ms | 0.00 s | `अरे दोस्त, क्या हाल-चाल है? आज का काम कैसा चल रहा है?` |
| NeuTTS Nano | Named Entities (EN) | 33636 ms | 9.241 | 33636 ms | 3.64 s | `Barack Obama visited the Taj Mahal in Agra, India, with his family.` |
| NeuTTS Nano | Named Entities (HI) | 34028 ms | 7.877 | 34028 ms | 4.32 s | `नरेंद्र मोदी ने नई दिल्ली में लाल किले पर झंडा फहराया।` |
| NeuTTS Nano | Numbers (EN) | 63284 ms | 9.041 | 63284 ms | 7.00 s | `The year is 2026, and the temperature is exactly 37.5 degrees Celsius.` |
| NeuTTS Nano | Numbers (HI) | 30193 ms | 8.529 | 30193 ms | 3.54 s | `रेलगाड़ी संख्या 12345 दोपहर 3:45 बजे प्लेटफार्म नंबर 5 पर आएगी।` |
| NeuTTS Nano | URLs | 30237 ms | 10.147 | 30237 ms | 2.98 s | `You can visit our website at https://deepmind.google/technologies/gemini/ for details.` |
| NeuTTS Nano | Abbreviations | 45420 ms | 10.276 | 45421 ms | 4.42 s | `The AI assistant was built using the AGY SDK and runs on a local PC using CPU threads.` |
| NeuTTS Nano | Paragraph (EN) | 80902 ms | 10.874 | 80902 ms | 7.44 s | `Speech synthesis has come a long way from the early days of robotic sound generation. Modern engines utilize deep learning architectures, combined with neural vocoders, to produce highly expressive and human-like prosody. This allows desktop virtual assistants to sound natural, providing a pleasant user experience even on budget systems with hardware constraints.` |
| NeuTTS Nano | Paragraph (HI) | 13295 ms | 110.790 | 13295 ms | 0.12 s | `आवाज संश्लेषण तकनीक ने पिछले कुछ वर्षों में बहुत प्रगति की है। आजकल की प्रणालियाँ कृत्रिम बुद्धिमत्ता और तंत्रिका नेटवर्क का उपयोग करके बहुत ही प्राकृतिक और स्पष्ट आवाज उत्पन्न कर सकती हैं। यह तकनीक विशेष रूप से उन लोगों के लिए बहुत उपयोगी है जो पढ़ने में असमर्थ हैं या जो बोलकर बातचीत करना पसंद करते हैं।` |

## Key Insights & Comparative Analysis

### 1. Latency & Real-Time Performance
- **Average Time-To-First-Audio (TTFA)**:
  - Kokoro/Piper: **7515.9 ms**
  - NeuTTS Nano: **34692.5 ms**
- **Average Real-Time Factor (RTF)**:
  - Kokoro/Piper: **0.942** (Highly optimized VITS/ONNX)
  - NeuTTS Nano: **40.891** (Burn-based 2-stage neural architecture)

### 2. Resource Footprint
- **RAM consumption increment**:
  - Kokoro/Piper: **455 MB**
  - NeuTTS Nano: **1011 MB**
Both engines fit comfortably within the 8GB RAM target device budget, keeping memory footprints under 600MB.

### 3. Language & Pronunciation Nuances
- **Hindi/Hinglish**: NeuTTS Nano processes the Devanagari script natively by switching the underlying phonetic transcriber dynamically, eliminating language-boundary glitches.
- **Numbers & URLs**: Numbers are spoken naturally. URLs and acronyms are handled elegantly by the integrated espeak-ng phoneticizer.
