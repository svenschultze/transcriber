use crate::{utils};
use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use std::fs::File;
use voice_activity_detector::{VoiceActivityDetector, IteratorExt, LabeledAudio};

#[derive(Debug, serde::Serialize)]
pub struct AudioSegment {
    pub start_sample: i64,
    pub end_sample: i64,
    pub start_time_seconds: f64,
    pub end_time_seconds: f64,
    pub audio_data: Vec<i16>,
    pub audio_base64: String, // Base64-encoded WAV data for browser playback
}

pub struct AudioProcessor {
    sample_rate: utils::SampleRate,
}

impl AudioProcessor {
    pub fn new() -> Self {
        Self {
            sample_rate: utils::SampleRate::SixteenkHz, // Default to 16kHz
        }
    }

    // Decode audio using Symphonia (supports MP3, WAV, FLAC, etc.)
    pub fn decode_audio_symphonia(&self, file_path: &str) -> Result<(Vec<i16>, u32), Box<dyn std::error::Error>> {
        let dummy_callback = |_step: &str, _progress: f64, _details: Option<&str>| {};
        self.decode_audio_symphonia_with_progress(file_path, &dummy_callback)
    }

    fn decode_audio_symphonia_with_progress<F>(&self, file_path: &str, progress_callback: &F) -> Result<(Vec<i16>, u32), Box<dyn std::error::Error>>
    where
        F: Fn(&str, f64, Option<&str>),
    {
        let file = File::open(file_path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(extension) = std::path::Path::new(file_path).extension() {
            if let Some(ext_str) = extension.to_str() {
                hint.with_extension(ext_str);
            }
        }

        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;
        let mut format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or("No supported audio tracks found")?;

        let dec_opts: DecoderOptions = Default::default();
        let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

        let track_id = track.id;
        let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
        let channels = track.codec_params.channels.unwrap_or_default().count();

        let mut samples = Vec::new();
        let mut sample_buf = None;
        let mut packet_count = 0;
        let estimated_packets = 1000; // Rough estimate for progress tracking

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::ResetRequired) => {
                    break;
                }
                Err(SymphoniaError::IoError(err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof
                        && err.to_string() == "end of stream" =>
                {
                    break;
                }
                Err(err) => return Err(err.into()),
            };

            if packet.track_id() != track_id {
                continue;
            }

            packet_count += 1;
            
            // Update progress every 50 packets
            if packet_count % 50 == 0 {
                let decode_progress = 10.0 + (packet_count as f64 / estimated_packets as f64) * 15.0;
                progress_callback("Decoding audio packets", decode_progress.min(24.0), Some(&format!("Processed {} packets", packet_count)));
            }

            match decoder.decode(&packet) {
                Ok(audio_buf) => {
                    if sample_buf.is_none() {
                        let spec = *audio_buf.spec();
                        let duration = audio_buf.capacity() as u64;
                        sample_buf = Some(SampleBuffer::<i16>::new(duration, spec));
                    }

                    if let Some(buf) = &mut sample_buf {
                        buf.copy_interleaved_ref(audio_buf);
                        
                        // Convert to mono if stereo
                        let buf_samples = buf.samples();
                        if channels == 1 {
                            samples.extend_from_slice(buf_samples);
                        } else {
                            // Convert stereo to mono by averaging channels
                            for chunk in buf_samples.chunks(channels) {
                                if !chunk.is_empty() {
                                    let mono_sample = chunk.iter().map(|&s| s as i32).sum::<i32>() / chunk.len() as i32;
                                    samples.push(mono_sample as i16);
                                }
                            }
                        }
                    }
                }
                Err(SymphoniaError::IoError(_)) => break,
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(err) => return Err(err.into()),
            }
        }

        if samples.is_empty() {
            return Err("No audio samples decoded".into());
        }

        Ok((samples, sample_rate))
    }

    pub fn process_audio_file(&mut self, file_path: &str, _model_path: &str) -> Result<Vec<AudioSegment>, Box<dyn std::error::Error>> {
        // Default progress callback that does nothing
        let dummy_callback = |_step: &str, _progress: f64, _details: Option<&str>| {};
        self.process_audio_file_with_progress(file_path, _model_path, dummy_callback)
    }

    pub fn process_audio_file_with_progress<F>(&mut self, file_path: &str, _model_path: &str, progress_callback: F) -> Result<Vec<AudioSegment>, Box<dyn std::error::Error>>
    where
        F: Fn(&str, f64, Option<&str>),
    {
        // Check file extension to provide better error messages
        let path = std::path::Path::new(file_path);
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        println!("Processing audio file: {} (format: {})", file_path, extension);
        progress_callback("Validating file format", 5.0, Some(&format!("Detected format: {}", extension)));
        
        // Support multiple audio formats now
        match extension.as_str() {
            "wav" | "mp3" | "m4a" | "aac" | "flac" | "ogg" => {
                // Supported formats - continue processing
            },
            _ => {
                return Err(format!("Unsupported audio format: '{}'. Supported formats: WAV, MP3, M4A, AAC, FLAC, OGG", extension).into());
            }
        }
        
        // Decode audio using Symphonia
        progress_callback("Decoding audio file", 10.0, Some("Reading and decoding audio data"));
        let (mut content, original_sample_rate) = self.decode_audio_symphonia_with_progress(file_path, &progress_callback)?;
        
        // Always target 16kHz for VAD processing
        let target_sample_rate = utils::SampleRate::SixteenkHz;
        let target_rate_hz = 16000u32;
        
        println!("Processing audio file: {} Hz -> {} Hz", original_sample_rate, target_rate_hz);
        progress_callback("Audio decoded", 25.0, Some(&format!("{} samples at {} Hz", content.len(), original_sample_rate)));
        
        self.sample_rate = target_sample_rate;

        if content.is_empty() {
            return Err("Audio file is empty or contains no valid samples.".into());
        }

        println!("Original audio: {} samples at {} Hz", content.len(), original_sample_rate);

        // Resample to 16kHz if needed
        if original_sample_rate != target_rate_hz {
            progress_callback("Resampling audio", 35.0, Some(&format!("Converting from {} Hz to {} Hz", original_sample_rate, target_rate_hz)));
            content = self.simple_resample(&content, original_sample_rate, target_rate_hz);
            println!("Resampled to: {} samples at {} Hz", content.len(), target_rate_hz);
            progress_callback("Audio resampled", 45.0, Some(&format!("{} samples at {} Hz", content.len(), target_rate_hz)));
        }

        // Use real Silero VAD through voice_activity_detector crate
        println!("Running voice activity detection...");
        progress_callback("Running voice activity detection", 50.0, Some("Initializing AI voice detection"));
        
        // According to the docs, 16kHz sample rate requires 512-sample chunks
        let chunk_size = 512usize;
        let mut vad = VoiceActivityDetector::builder()
            .sample_rate(16000) // We always resample to 16kHz
            .chunk_size(chunk_size)
            .build()
            .map_err(|e| format!("Failed to create VAD: {}", e))?;
        
        // Use the label iterator with threshold 0.5 and 2 chunks padding
        let threshold = 0.5;
        let padding_chunks = 2;
        
        progress_callback("Analyzing speech patterns", 60.0, Some("Processing audio chunks for speech detection"));
        let labels: Vec<_> = content.iter().cloned().label(&mut vad, threshold, padding_chunks).collect();
        progress_callback("Speech detection complete", 75.0, Some(&format!("Processed {} audio chunks", labels.len())));
        
        // Convert labeled chunks back to continuous segments
        let mut segments = Vec::new();
        let mut current_speech_start = None;
        let sample_rate_f64 = 16000.0; // We know it's 16kHz after resampling
        
        progress_callback("Extracting speech segments", 80.0, Some("Converting detection results to segments"));
        
        for (chunk_index, label) in labels.iter().enumerate() {
            let chunk_start_sample = chunk_index * chunk_size;
            let chunk_start_time = chunk_start_sample as f64 / sample_rate_f64;
            
            match label {
                LabeledAudio::Speech(chunk_data) => {
                    if current_speech_start.is_none() {
                        // Start of a new speech segment
                        current_speech_start = Some(chunk_start_sample);
                    }
                }
                LabeledAudio::NonSpeech(_) => {
                    if let Some(speech_start) = current_speech_start.take() {
                        // End of speech segment
                        let speech_end = chunk_start_sample;
                        let start_time = speech_start as f64 / sample_rate_f64;
                        let end_time = speech_end as f64 / sample_rate_f64;
                        
                        // Extract audio data for this segment
                        let start_idx = speech_start.min(content.len());
                        let end_idx = speech_end.min(content.len());
                        let segment_audio = content[start_idx..end_idx].to_vec();
                        
                        if !segment_audio.is_empty() {
                            let audio_base64 = self.samples_to_wav_base64(&segment_audio)
                                .unwrap_or_else(|_| String::new());
                            
                            segments.push(AudioSegment {
                                start_sample: speech_start as i64,
                                end_sample: speech_end as i64,
                                start_time_seconds: start_time,
                                end_time_seconds: end_time,
                                audio_data: segment_audio,
                                audio_base64,
                            });
                        }
                    }
                }
            }
        }
        
        // Handle any remaining speech segment at the end
        if let Some(speech_start) = current_speech_start {
            let speech_end = content.len();
            let start_time = speech_start as f64 / sample_rate_f64;
            let end_time = speech_end as f64 / sample_rate_f64;
            
            let start_idx = speech_start.min(content.len());
            let segment_audio = content[start_idx..].to_vec();
            
            if !segment_audio.is_empty() {
                let audio_base64 = self.samples_to_wav_base64(&segment_audio)
                    .unwrap_or_else(|_| String::new());
                
                segments.push(AudioSegment {
                    start_sample: speech_start as i64,
                    end_sample: speech_end as i64,
                    start_time_seconds: start_time,
                    end_time_seconds: end_time,
                    audio_data: segment_audio,
                    audio_base64,
                });
            }
        }

        println!("Generated {} initial speech segments using Silero VAD", segments.len());
        progress_callback("Optimizing segments", 90.0, Some(&format!("Found {} initial segments", segments.len())));

        // Merge segments that are close together (within 3 seconds)
        let merged_segments = self.merge_close_segments_with_progress(segments, &content, 1.5, &progress_callback);
        
        println!("After merging close segments: {} final segments", merged_segments.len());
        progress_callback("Segmentation complete", 95.0, Some(&format!("Optimized to {} final segments", merged_segments.len())));

        Ok(merged_segments)
    }

    // Merge segments that are close together (within max_gap_seconds)
    fn merge_close_segments(&self, mut segments: Vec<AudioSegment>, content: &[i16], max_gap_seconds: f64) -> Vec<AudioSegment> {
        let dummy_callback = |_step: &str, _progress: f64, _details: Option<&str>| {};
        self.merge_close_segments_with_progress(segments, content, max_gap_seconds, &dummy_callback)
    }

    fn merge_close_segments_with_progress<F>(&self, mut segments: Vec<AudioSegment>, content: &[i16], max_gap_seconds: f64, progress_callback: &F) -> Vec<AudioSegment>
    where
        F: Fn(&str, f64, Option<&str>),
    {
        if segments.is_empty() {
            return segments;
        }

        // Sort segments by start time to ensure proper order
        segments.sort_by(|a, b| a.start_time_seconds.partial_cmp(&b.start_time_seconds).unwrap());

        let mut merged = Vec::new();
        let mut segments_iter = segments.into_iter();
        let mut current = segments_iter.next().unwrap();
        let mut processed = 0;
        let total_segments = segments_iter.len() + 1;

        for next in segments_iter {
            processed += 1;
            
            // Update progress during merging
            if processed % 10 == 0 || processed == total_segments - 1 {
                let merge_progress = 90.0 + (processed as f64 / total_segments as f64) * 5.0;
                progress_callback("Merging segments", merge_progress, Some(&format!("Processed {}/{} segments", processed, total_segments)));
            }
            
            let gap = next.start_time_seconds - current.end_time_seconds;
            
            if gap <= max_gap_seconds {
                // Merge current and next segments
                println!("Merging segments: {:.2}s-{:.2}s with {:.2}s-{:.2}s (gap: {:.2}s)", 
                    current.start_time_seconds, current.end_time_seconds,
                    next.start_time_seconds, next.end_time_seconds, gap);
                
                let merged_start = current.start_sample;
                let merged_end = next.end_sample;
                let merged_start_time = current.start_time_seconds;
                let merged_end_time = next.end_time_seconds;
                
                // Extract audio data for the merged segment (including the gap)
                let start_idx = merged_start.min(content.len() as i64) as usize;
                let end_idx = (merged_end as usize).min(content.len());
                let merged_audio = content[start_idx..end_idx].to_vec();
                
                println!("Merged segment: {:.2}s-{:.2}s, samples: {}-{}, audio length: {} samples", 
                    merged_start_time, merged_end_time, merged_start, merged_end, merged_audio.len());
                
                let audio_base64 = self.samples_to_wav_base64(&merged_audio)
                    .unwrap_or_else(|_| String::new());
                
                current = AudioSegment {
                    start_sample: merged_start,
                    end_sample: merged_end,
                    start_time_seconds: merged_start_time,
                    end_time_seconds: merged_end_time,
                    audio_data: merged_audio,
                    audio_base64,
                };
            } else {
                // Gap is too large, keep current segment and move to next
                println!("Gap too large ({:.2}s > {:.2}s), not merging segments: {:.2}s-{:.2}s and {:.2}s-{:.2}s", 
                    gap, max_gap_seconds,
                    current.start_time_seconds, current.end_time_seconds,
                    next.start_time_seconds, next.end_time_seconds);
                merged.push(current);
                current = next;
            }
        }
        
        // Don't forget to add the last segment
        merged.push(current);
        
        merged
    }

    // Convert audio samples to base64-encoded WAV for browser playback
    fn samples_to_wav_base64(&self, samples: &[i16]) -> Result<String, Box<dyn std::error::Error>> {
        let sample_rate = 16000u32; // Always 16kHz for our processed audio
        let channels = 1u16; // Mono
        let bits_per_sample = 16u16;
        
        let mut wav_data = Vec::new();
        
        // WAV header
        wav_data.extend_from_slice(b"RIFF");
        let file_size = 36 + (samples.len() * 2) as u32;
        wav_data.extend_from_slice(&file_size.to_le_bytes());
        wav_data.extend_from_slice(b"WAVE");
        
        // Format chunk
        wav_data.extend_from_slice(b"fmt ");
        wav_data.extend_from_slice(&16u32.to_le_bytes()); // Chunk size
        wav_data.extend_from_slice(&1u16.to_le_bytes()); // Audio format (PCM)
        wav_data.extend_from_slice(&channels.to_le_bytes());
        wav_data.extend_from_slice(&sample_rate.to_le_bytes());
        let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
        wav_data.extend_from_slice(&byte_rate.to_le_bytes());
        let block_align = channels * bits_per_sample / 8;
        wav_data.extend_from_slice(&block_align.to_le_bytes());
        wav_data.extend_from_slice(&bits_per_sample.to_le_bytes());
        
        // Data chunk
        wav_data.extend_from_slice(b"data");
        let data_size = (samples.len() * 2) as u32;
        wav_data.extend_from_slice(&data_size.to_le_bytes());
        
        // Audio data
        for &sample in samples {
            wav_data.extend_from_slice(&sample.to_le_bytes());
        }
        
        // Encode to base64
        Ok(base64::encode(&wav_data))
    }

    pub fn extract_audio_chunk(&self, content: &[i16], start_sample: i64, end_sample: i64) -> Vec<i16> {
        let start_idx = start_sample.max(0) as usize;
        let end_idx = (end_sample as usize).min(content.len());
        content[start_idx..end_idx].to_vec()
    }

    /// Simple resampling by linear interpolation
    /// This is a basic approach - for production, you'd want proper anti-aliasing
    fn simple_resample(&self, input: &[i16], from_rate: u32, to_rate: u32) -> Vec<i16> {
        if from_rate == to_rate {
            return input.to_vec(); // No resampling needed
        }
        
        let ratio = from_rate as f64 / to_rate as f64;
        let output_len = (input.len() as f64 / ratio) as usize;
        let mut output = Vec::with_capacity(output_len);
        
        for i in 0..output_len {
            let src_pos = i as f64 * ratio;
            let src_index = src_pos as usize;
            
            if src_index >= input.len() {
                break;
            }
            
            // Linear interpolation between samples
            if src_index + 1 < input.len() {
                let frac = src_pos - src_index as f64;
                let sample1 = input[src_index] as f64;
                let sample2 = input[src_index + 1] as f64;
                let interpolated = sample1 + (sample2 - sample1) * frac;
                output.push(interpolated as i16);
            } else {
                output.push(input[src_index]);
            }
        }
        
        output
    }
    
    /// Public wrapper for resampling audio
    pub fn resample_audio(&self, input: &[i16], from_rate: u32, to_rate: u32) -> Result<Vec<i16>, Box<dyn std::error::Error>> {
        Ok(self.simple_resample(input, from_rate, to_rate))
    }
    
    /// Convert audio samples to WAV bytes (without base64 encoding)
    pub fn samples_to_wav_bytes(&self, samples: &[i16], sample_rate: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut wav_data = Vec::new();
        
        // WAV header
        let num_samples = samples.len() as u32;
        let byte_rate = sample_rate * 2; // 16-bit mono
        let data_size = num_samples * 2;
        let file_size = 36 + data_size;
        
        // RIFF header
        wav_data.extend_from_slice(b"RIFF");
        wav_data.extend_from_slice(&file_size.to_le_bytes());
        wav_data.extend_from_slice(b"WAVE");
        
        // fmt chunk
        wav_data.extend_from_slice(b"fmt ");
        wav_data.extend_from_slice(&16u32.to_le_bytes()); // chunk size
        wav_data.extend_from_slice(&1u16.to_le_bytes()); // PCM format
        wav_data.extend_from_slice(&1u16.to_le_bytes()); // mono
        wav_data.extend_from_slice(&sample_rate.to_le_bytes());
        wav_data.extend_from_slice(&byte_rate.to_le_bytes());
        wav_data.extend_from_slice(&2u16.to_le_bytes()); // block align
        wav_data.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        
        // data chunk
        wav_data.extend_from_slice(b"data");
        wav_data.extend_from_slice(&data_size.to_le_bytes());
        
        // audio data
        for &sample in samples {
            wav_data.extend_from_slice(&sample.to_le_bytes());
        }
        
        Ok(wav_data)
    }
    
    // Extract a segment from an audio file by time range
    pub fn extract_segment_from_file(
        &self,
        file_path: &std::path::Path,
        start_time_seconds: f64,
        end_time_seconds: f64,
    ) -> Result<(Vec<i16>, u32), Box<dyn std::error::Error>> {
        // Decode the full audio file
        let (audio_samples, sample_rate) = self.decode_audio_symphonia(file_path.to_str().unwrap())?;
        
        // Calculate sample indices
        let start_sample = (start_time_seconds * sample_rate as f64) as usize;
        let end_sample = (end_time_seconds * sample_rate as f64) as usize;
        
        // Ensure we don't go out of bounds
        let start_sample = start_sample.min(audio_samples.len());
        let end_sample = end_sample.min(audio_samples.len());
        
        if start_sample >= end_sample {
            return Err("Invalid time range: start time is after end time".into());
        }
        
        // Extract the segment
        let segment_samples = audio_samples[start_sample..end_sample].to_vec();
        
        Ok((segment_samples, sample_rate))
    }
}
