// Modules
mod audio_processing;
mod silero;
mod utils;
mod vad_iter;

use audio_processing::{AudioProcessor, AudioSegment};
use serde::{Serialize, Deserialize};
use tauri::Emitter;

#[derive(Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    pub step: String,
    pub progress: f64, // 0.0 to 100.0
    pub details: Option<String>,
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn save_audio_file(file_data: Vec<u8>, filename: String) -> Result<String, String> {
    use std::fs;
    use std::env;
    
    // Create a temporary directory for audio files
    let temp_dir = env::temp_dir().join("transcriber_audio");
    if !temp_dir.exists() {
        fs::create_dir_all(&temp_dir).map_err(|e| format!("Failed to create temp directory: {}", e))?;
    }
    
    // Create a unique filename
    let uuid = uuid::Uuid::new_v4();
    let file_extension = std::path::Path::new(&filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("wav");
    let temp_filename = format!("{}.{}", uuid, file_extension);
    let temp_path = temp_dir.join(temp_filename);
    
    // Save the file
    fs::write(&temp_path, file_data).map_err(|e| format!("Failed to write file: {}", e))?;
    
    Ok(temp_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn select_audio_file() -> Result<Option<String>, String> {
    // For now, return None since we need to implement this properly
    // This is a placeholder that can be expanded later
    Ok(None)
}

#[tauri::command]
async fn process_audio_vad(file_path: String, app_handle: tauri::AppHandle) -> Result<Vec<AudioSegment>, String> {
    // Check if file exists
    if !std::path::Path::new(&file_path).exists() {
        return Err(format!("File not found: {}", file_path));
    }

    // Create a progress callback
    let progress_callback = |step: &str, progress: f64, details: Option<&str>| {
        let update = ProgressUpdate {
            step: step.to_string(),
            progress,
            details: details.map(|s| s.to_string()),
        };
        
        // Emit progress event
        if let Err(e) = app_handle.emit("audio-processing-progress", &update) {
            eprintln!("Failed to emit progress event: {}", e);
        }
    };

    // Process the audio file with progress reporting
    let mut processor = AudioProcessor::new();
    
    match processor.process_audio_file_with_progress(&file_path, "mock_model_path", progress_callback) {
        Ok(segments) => {
            // Final progress update
            progress_callback("Processing complete", 100.0, Some(&format!("Found {} speech segments", segments.len())));
            Ok(segments)
        },
        Err(e) => Err(format!("Error processing audio file: {}", e))
    }
}

#[tauri::command]
async fn transcribe_audio(
    audio_base64: String, 
    segment_index: usize,
    api_key: String,
    base_url: String,
    model_name: String
) -> Result<String, String> {
    // Decode base64 to bytes
    let audio_bytes = base64::decode(&audio_base64)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;
    
    // Create multipart form
    let form = reqwest::multipart::Form::new()
        .part("file", reqwest::multipart::Part::bytes(audio_bytes)
            .file_name(format!("segment_{}.wav", segment_index))
            .mime_str("audio/wav")
            .map_err(|e| format!("Failed to set mime type: {}", e))?)
        .text("model", model_name);
        //.text("language", "en");
    
    // Create HTTP client
    let client = reqwest::Client::new();
    
    // Make the API request
    let response = client
        .post(&format!("{}/audio/transcriptions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;
    
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("API error {}: {}", status, error_text));
    }
    
    // Parse the response
    let result: serde_json::Value = response.json().await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    // Extract the transcription text
    let text = result.get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    
    Ok(text)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![greet, process_audio_vad, select_audio_file, save_audio_file, transcribe_audio])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
