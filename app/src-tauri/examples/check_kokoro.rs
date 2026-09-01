use std::path::Path;
fn main() {
    let model_path = Path::new("/home/addy/.vox/models/tts/kokoro");
    println!("Trying Kokoro at {:?}", model_path);
    let res = vox_lib::services::tts::KokoroEngine::new(model_path, 0, 1.0);
    match res {
        Ok(_) => println!("Kokoro init OK"),
        Err(e) => println!("Kokoro init FAILED: {}", e),
    }
}
