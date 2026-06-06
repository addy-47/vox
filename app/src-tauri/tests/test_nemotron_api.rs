use std::path::PathBuf;
use parakeet_rs::Nemotron;

#[test]
fn test_loading() {
    println!("=== Testing parakeet-rs Nemotron loading (from tests/) ===");
    let model_dir = std::path::PathBuf::from("/home/addy/.vox/models/stt/nvidia-nemotron-3.5");
    assert!(model_dir.exists(), "Error: Model directory {:?} does not exist!", model_dir);

    // Inspect inputs of encoder.onnx first
    let encoder_path = model_dir.join("encoder.onnx");
    if encoder_path.exists() {
        if let Ok(session) = ort::session::Session::builder().unwrap().commit_from_file(&encoder_path) {
            println!("Encoder inputs:");
            for input in session.inputs() {
                println!("  - Name: {}, Type: {:?}", input.name(), input.dtype());
            }
            println!("Encoder outputs:");
            for output in session.outputs() {
                println!("  - Name: {}, Type: {:?}", output.name(), output.dtype());
            }
        }
    }

    // Inspect inputs of decoder_joint.onnx
    let joint_path = model_dir.join("decoder_joint.onnx");
    if joint_path.exists() {
        if let Ok(session) = ort::session::Session::builder().unwrap().commit_from_file(&joint_path) {
            println!("Decoder Joint inputs:");
            for input in session.inputs() {
                println!("  - Name: {}, Type: {:?}", input.name(), input.dtype());
            }
        }
    }

    // Attempt loading via parakeet-rs
    let model = Nemotron::from_pretrained(&model_dir, None);
    match model {
        Ok(mut m) => {
            println!("Successfully loaded Nemotron model!");
            m.reset();
            println!("reset() method exists and compiles!");
            
            // Try calling transcribe_chunk to verify signature
            let result = m.transcribe_chunk(&[0.0; 8960]);
            match result {
                Ok(text) => println!("transcribe_chunk compiles successfully! Result: {:?}", text),
                Err(e) => panic!("transcribe_chunk failed: {:?}", e),
            }
        }
        Err(e) => {
            panic!("Failed to load Nemotron model: {:?}", e);
        }
    }
}
