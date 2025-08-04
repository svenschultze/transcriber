# Audio Transcription App with Silero VAD

This is a Tauri application that implements Voice Activity Detection (VAD) using the Silero VAD model for audio transcription.

## Features

- Voice Activity Detection using Silero VAD
- Audio file processing (WAV format, 8kHz or 16kHz)
- Speech segment extraction
- Frontend interface for file upload and results display

## Setup

### Prerequisites

1. **Rust** - Install from [rustup.rs](https://rustup.rs/)
2. **Node.js** - For the frontend
3. **Deno** - For running the frontend build
4. **Silero VAD Model** - Download the ONNX model file

### Getting the Silero VAD Model

1. Download the Silero VAD model from: https://github.com/snakers4/silero-vad
2. You need the `silero_vad.onnx` file
3. Place it in your project directory or set the `SILERO_MODEL_PATH` environment variable

### Installation

1. Clone this repository
2. Navigate to the project directory
3. Install dependencies:
   ```bash
   cd transcriber
   deno install
   ```

### Running the Application

1. **Development mode:**
   ```bash
   deno task tauri dev
   ```

2. **Build for production:**
   ```bash
   deno task tauri build
   ```

## Project Structure

```
src-tauri/
├── src/
│   ├── lib.rs              # Main Tauri application
│   ├── audio_processing.rs # Audio processing logic
│   ├── silero.rs          # Silero VAD implementation
│   ├── utils.rs           # Utility types and functions
│   ├── vad_iter.rs        # VAD iterator implementation
│   └── main.rs            # Entry point
├── Cargo.toml             # Rust dependencies
└── tauri.conf.json        # Tauri configuration
```

## Current Implementation Status

### ✅ Completed
- Basic Tauri app setup
- Silero VAD Rust implementation (adapted from the official example)
- Audio processing pipeline structure
- Frontend interface for file selection
- Mock VAD results for testing

### 🚧 In Progress
- Real VAD processing (requires model file)
- File dialog integration
- Audio file validation

### 📋 Todo
- OpenAI transcription API integration
- Drag and drop file upload
- Individual chunk editing
- Audio playback for segments
- Export functionality

## Testing

Currently, the app returns mock VAD results to test the pipeline. To test with real audio:

1. Download the Silero VAD model (`silero_vad.onnx`)
2. Set the `SILERO_MODEL_PATH` environment variable or place the model file in the project root
3. Uncomment the real VAD processing code in `lib.rs`
4. Prepare a WAV audio file (8kHz or 16kHz sample rate, 16-bit integer format)
5. Use the file selection interface to process the audio

## Dependencies

### Rust (Backend)
- `tauri` - Desktop app framework
- `ort` - ONNX Runtime for ML inference
- `ndarray` - N-dimensional arrays for audio processing
- `hound` - WAV file reading
- `serde` - Serialization
- `tokio` - Async runtime

### Frontend
- Vue 3
- TypeScript
- Vite

## API

### Tauri Commands

- `greet(name: string)` - Test command
- `process_audio_vad(file_path: string)` - Process audio file with VAD
- `select_audio_file()` - Open file dialog (placeholder)

## Troubleshooting

1. **Model not found**: Ensure the Silero VAD model file is in the correct location
2. **Audio format issues**: Only WAV files with 8kHz or 16kHz sample rates are supported
3. **Build errors**: Make sure all Rust dependencies are properly installed

## Next Steps

1. Download and integrate the Silero VAD model
2. Test with real audio files
3. Implement OpenAI transcription integration
4. Add drag-and-drop functionality
5. Implement segment editing features
