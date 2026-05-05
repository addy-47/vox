use ort::session::Session;
use anyhow::Result;

fn main() -> Result<()> {
    let models = vec![
        "assets/qwen3-asr/conv_frontend.onnx",
        "assets/qwen3-asr/encoder.int8.onnx",
        "assets/qwen3-asr/decoder.int8.onnx",
    ];

    for path in models {
        println!("--- {} ---", path);
        let session = Session::builder()?.commit_from_file(path)?;
        for input in session.inputs() {
            println!("  Input: {} {:?}", input.name(), input.input_type);
        }
        for output in session.outputs() {
            println!("  Output: {} {:?}", output.name(), output.output_type);
        }
    }
    Ok(())
}
