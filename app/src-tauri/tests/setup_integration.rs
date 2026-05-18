use std::path::PathBuf;
use std::fs;
use tempfile::tempdir;
use vox_lib::setup::manifest::VoxManifest;
use vox_lib::setup::model_manager::ModelManager;
use vox_lib::utils::paths;

#[tokio::test]
async fn test_manifest_fetch_and_parse() {
    // 1. Fetch real manifest
    let manifest = VoxManifest::fetch().await.expect("Failed to fetch manifest");
    
    // 2. Verify basic fields
    assert!(!manifest.version.is_empty());
    assert!(manifest.models.len() > 0);
    
    let required_space = manifest.calculate_required_space();
    println!("Required space: {} bytes", required_space);
    assert!(required_space > 0);
}

#[tokio::test]
async fn test_model_manager_download_sandbox() {
    // 1. Setup Sandbox
    let tmp_dir = tempdir().expect("Failed to create temp dir");
    let sandbox_path = tmp_dir.path().to_path_buf();
    println!("Sandbox path: {:?}", sandbox_path);
    
    // Initialize paths with sandbox root
    paths::init_with_root(sandbox_path.clone());
    paths::ensure_dirs().expect("Failed to create sandbox dirs");
    
    // 2. Initialize ModelManager (Mock app handle)
    let manager = ModelManager::new(None);
    
    // 3. Fetch manifest to get correct URLs and metadata
    let manifest = VoxManifest::fetch().await.expect("Failed to fetch manifest");
    let base_url = "https://huggingface.co/addyo07/vox-models/resolve/main";
    
    // 4. Test downloading a SMALL file: ten_vad.onnx (id: ten_vad)
    let vad_entry = manifest.get_model("ten_vad").expect("ten_vad not found in manifest");
    
    println!("Downloading {} ({} bytes)...", vad_entry.id, vad_entry.size_bytes);
    
    let result = manager.setup_model(vad_entry, base_url, &paths::models_dir()).await;
    
    if let Err(e) = result {
        panic!("Surgical setup failed for ten_vad: {}", e);
    }
    
    // 5. Verify file existence and marker in sandbox
    let vad_path = paths::model_dir("vad").join("ten_vad.onnx");
    assert!(vad_path.exists(), "ten_vad.onnx was not downloaded to {:?}", vad_path);
    
    let marker_path = vad_path.with_extension("verified");
    assert!(marker_path.exists(), "Verified marker for ten_vad was not created");
    
    // Verify marker content
    let marker_json = fs::read_to_string(&marker_path).expect("Failed to read marker");
    println!("Marker content: {}", marker_json);
    assert!(marker_json.contains(&vad_entry.sha256));
    
    println!("Surgical sandbox verification successful!");
}
