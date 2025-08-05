// Modules
mod audio_processing;
mod utils;

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
async fn save_audio_file_chunked(chunk_data: Vec<u8>, chunk_index: usize, total_chunks: usize, filename: String, session_id: String) -> Result<String, String> {
    use std::fs;
    use std::env;
    use std::fs::OpenOptions;
    use std::io::Write;
    
    // Create a temporary directory for audio files
    let temp_dir = env::temp_dir().join("transcriber_audio");
    if !temp_dir.exists() {
        fs::create_dir_all(&temp_dir).map_err(|e| format!("Failed to create temp directory: {}", e))?;
    }
    
    // Create session-based filename
    let temp_filename = format!("{}_{}", session_id, filename);
    let temp_path = temp_dir.join(temp_filename);
    
    // Append chunk to file
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&temp_path)
        .map_err(|e| format!("Failed to open temp file: {}", e))?;
    
    file.write_all(&chunk_data).map_err(|e| format!("Failed to write chunk: {}", e))?;
    file.flush().map_err(|e| format!("Failed to flush file: {}", e))?;
    
    // If this is the last chunk, process the complete file
    if chunk_index == total_chunks - 1 {
        // Convert to 16kHz WAV format
        let mut processor = AudioProcessor::new();
        let (audio_samples, original_sample_rate) = processor.decode_audio_symphonia(&temp_path.to_string_lossy())
            .map_err(|e| format!("Failed to decode audio: {}", e))?;
        
        // Resample to 16kHz if needed
        let target_sample_rate = 16000;
        let resampled_audio = if original_sample_rate != target_sample_rate {
            processor.resample_audio(&audio_samples, original_sample_rate, target_sample_rate)
                .map_err(|e| format!("Failed to resample audio: {}", e))?
        } else {
            audio_samples
        };
        
        // Create the final processed filename
        let uuid = uuid::Uuid::new_v4();
        let processed_filename = format!("{}_processed.wav", uuid);
        let processed_path = temp_dir.join(processed_filename);
        
        // Save as WAV with 16kHz
        let wav_data = processor.samples_to_wav_bytes(&resampled_audio, target_sample_rate)
            .map_err(|e| format!("Failed to create WAV data: {}", e))?;
        
        fs::write(&processed_path, wav_data).map_err(|e| format!("Failed to write processed file: {}", e))?;
        
        // Clean up the original temporary file
        let _ = fs::remove_file(temp_path);
        
        Ok(processed_path.to_string_lossy().to_string())
    } else {
        // Return temporary status for intermediate chunks
        Ok(format!("chunk_{}_of_{}_received", chunk_index + 1, total_chunks))
    }
}

#[tauri::command]
async fn save_audio_file(file_data: Vec<u8>, filename: String) -> Result<String, String> {
    use std::fs;
    use std::env;
    use std::io::Cursor;
    
    // Create a temporary directory for audio files
    let temp_dir = env::temp_dir().join("transcriber_audio");
    if !temp_dir.exists() {
        fs::create_dir_all(&temp_dir).map_err(|e| format!("Failed to create temp directory: {}", e))?;
    }
    
    // Create a unique filename for the original file
    let uuid = uuid::Uuid::new_v4();
    let file_extension = std::path::Path::new(&filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("wav");
    let original_temp_filename = format!("{}_original.{}", uuid, file_extension);
    let original_temp_path = temp_dir.join(original_temp_filename);
    
    // Save the original file temporarily
    fs::write(&original_temp_path, file_data).map_err(|e| format!("Failed to write original file: {}", e))?;
    
    // Convert to 16kHz MP3 using the audio processor
    let mut processor = AudioProcessor::new();
    let (audio_samples, original_sample_rate) = processor.decode_audio_symphonia(&original_temp_path.to_string_lossy())
        .map_err(|e| format!("Failed to decode audio: {}", e))?;
    
    // Resample to 16kHz if needed
    let target_sample_rate = 16000;
    let resampled_audio = if original_sample_rate != target_sample_rate {
        processor.resample_audio(&audio_samples, original_sample_rate, target_sample_rate)
            .map_err(|e| format!("Failed to resample audio: {}", e))?
    } else {
        audio_samples
    };
    
    // Create the final MP3 filename
    let mp3_filename = format!("{}.mp3", uuid);
    let mp3_path = temp_dir.join(mp3_filename);
    
    // Save as MP3 (for now we'll save as WAV since we don't have MP3 encoder, but with 16kHz)
    // TODO: Add proper MP3 encoding library
    let wav_data = processor.samples_to_wav_bytes(&resampled_audio, target_sample_rate)
        .map_err(|e| format!("Failed to create WAV data: {}", e))?;
    
    fs::write(&mp3_path, wav_data).map_err(|e| format!("Failed to write processed file: {}", e))?;
    
    // Clean up the original temporary file
    let _ = fs::remove_file(original_temp_path);
    
    Ok(mp3_path.to_string_lossy().to_string())
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
async fn convert_audio_to_base64(file_path: String) -> Result<String, String> {
    // Read the entire audio file
    let audio_bytes = std::fs::read(&file_path)
        .map_err(|e| format!("Failed to read audio file: {}", e))?;
    
    // Encode to base64
    let base64_string = base64::encode(&audio_bytes);
    
    Ok(base64_string)
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

#[tauri::command]
async fn check_file_exists(file_path: String) -> Result<bool, String> {
    use std::path::Path;
    
    let path = Path::new(&file_path);
    Ok(path.exists() && path.is_file())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![greet, process_audio_vad, select_audio_file, save_audio_file, save_audio_file_chunked, transcribe_audio, convert_audio_to_base64, check_file_exists])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
