<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const audioFile = ref<File | null>(null);
const vadResults = ref<any[]>([]);
const isProcessing = ref(false);
const errorMsg = ref("");
const isTranscribing = ref(false);
const transcriptionStatus = ref("");
const currentAudio = ref<HTMLAudioElement | null>(null);
const isCtrlPressed = ref(false);
const projectName = ref("Untitled Project");
const showFileMenu = ref(false);
const showEditMenu = ref(false);
const showHelpMenu = ref(false);
const originalAudioBase64 = ref<string>("");
const showAudioPlayer = ref(false);
const currentSegmentInfo = ref<string>("");
const currentTime = ref(0);
const duration = ref(0);
const isPlaying = ref(false);
const currentHighlightedSegment = ref<number>(-1);

// Noscribe import state
const showAudioWarning = ref(false);
const missingAudioPath = ref("");
const audioLoadingProgress = ref(0);
const isLoadingAudio = ref(false);

// Settings
const showSettings = ref(false);
const apiKey = ref("sk-...");
const baseUrl = ref("https://api.openai.com/v1");
const modelName = ref("whisper-1");

// Progress tracking
const processingProgress = ref(0);
const processingStep = ref("");
const processingSteps = [
  "Reading audio file...",
  "Decoding audio format...", 
  "Resampling audio...",
  "Running voice detection...",
  "Analyzing speech segments...",
  "Merging segments...",
  "Finalizing results..."
];

// Track Ctrl key state for visual feedback
function handleKeyDown(event: KeyboardEvent) {
  if (event.ctrlKey) {
    isCtrlPressed.value = true;
    
    // Handle keyboard shortcuts
    if (event.key === 's') {
      event.preventDefault();
      saveProject();
    } else if (event.key === 'o') {
      event.preventDefault();
      document.getElementById('load-project')?.click();
    } else if (event.key === 'e') {
      event.preventDefault();
      if (event.shiftKey) {
        exportAsMarkdown();
      } else {
        exportAsText();
      }
    }
  }
}

function handleKeyUp(event: KeyboardEvent) {
  if (!event.ctrlKey) {
    isCtrlPressed.value = false;
  }
}

// Add event listeners when component mounts
import { onMounted, onUnmounted } from "vue";

onMounted(() => {
  document.addEventListener('keydown', handleKeyDown);
  document.addEventListener('keyup', handleKeyUp);
  
  // Load settings from localStorage
  loadSettings();
  
  // Listen for audio processing progress events
  listen('audio-processing-progress', (event) => {
    const progressData = event.payload as { step: string; progress: number; details?: string };
    processingStep.value = progressData.step;
    processingProgress.value = progressData.progress;
    
    if (progressData.details) {
      console.log(`Progress: ${progressData.step} (${progressData.progress}%) - ${progressData.details}`);
    }
  });
});

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeyDown);
  document.removeEventListener('keyup', handleKeyUp);
});

// Settings management
function loadSettings() {
  const savedApiKey = localStorage.getItem('transcriber-api-key');
  const savedBaseUrl = localStorage.getItem('transcriber-base-url');
  const savedModelName = localStorage.getItem('transcriber-model-name');
  
  if (savedApiKey) apiKey.value = savedApiKey;
  if (savedBaseUrl) baseUrl.value = savedBaseUrl;
  if (savedModelName) modelName.value = savedModelName;
}

function saveSettings() {
  localStorage.setItem('transcriber-api-key', apiKey.value);
  localStorage.setItem('transcriber-base-url', baseUrl.value);
  localStorage.setItem('transcriber-model-name', modelName.value);
  showSettings.value = false;
}

function resetSettings() {
  apiKey.value = "sk-...";
  baseUrl.value = "https://api.openai.com/v1";
  modelName.value = "whisper-1";
}

// Noscribe HTML import function
async function importNoscribeFile(event: Event) {
  const target = event.target as HTMLInputElement;
  if (!target.files || target.files.length === 0) return;
  
  const htmlFile = target.files[0];
  
  try {
    // Read the HTML file content
    const htmlContent = await htmlFile.text();
    
    // Parse the HTML
    const parser = new DOMParser();
    const doc = parser.parseFromString(htmlContent, 'text/html');
    
    // Extract audio source from meta tag
    const audioSourceMeta = doc.querySelector('meta[name="audio_source"]');
    const audioSourcePath = audioSourceMeta?.getAttribute('content');
    
    if (!audioSourcePath) {
      errorMsg.value = "No audio source found in the HTML file";
      return;
    }
    
    // Extract transcript content from the body
    const bodyContent = doc.body;
    const segments = parseNoscribeTranscript(bodyContent);
    
    // Set project name from filename
    const fileName = htmlFile.name;
    const nameWithoutExtension = fileName.substring(0, fileName.lastIndexOf('.')) || fileName;
    projectName.value = nameWithoutExtension;
    
    // Clear current data
    vadResults.value = [];
    originalAudioBase64.value = "";
    audioFile.value = null;
    
    // Set the extracted segments
    vadResults.value = segments;
    
    transcriptionStatus.value = `Imported ${segments.length} segments from noscribe file.`;
    
    // Try to load the audio file automatically first
    await tryLoadAudioFile(audioSourcePath);
    
  } catch (error) {
    errorMsg.value = `Error importing noscribe file: ${error}`;
    console.error("Import error:", error);
  }
  
  // Clear the input
  target.value = '';
}

async function tryLoadAudioFile(audioPath: string) {
  try {
    // First, try to check if the file exists using Tauri's file system API
    const fileExists = await invoke('check_file_exists', { filePath: audioPath });
    
    if (fileExists) {
      // File exists, try to load it
      transcriptionStatus.value = "Loading referenced audio file...";
      
      try {
        // Use the chunked file processing to load the existing file
        const result = await invoke('convert_audio_to_base64', {
          filePath: audioPath,
        });
        
        if (typeof result === 'string') {
          originalAudioBase64.value = result;
          transcriptionStatus.value = "Audio loaded successfully. Ready for playback.";
          
          // Create a mock file object for display purposes
          const fileName = audioPath.split(/[\\/]/).pop() || 'audio.wav';
          // We can't create a real File object from a path, but we can store the filename
          // The audio playback will work with originalAudioBase64.value
          
          // Create audio player
          createAudioPlayer();
          
          // Hide any warning that might be showing
          showAudioWarning.value = false;
        }
      } catch (error) {
        console.error("Error loading audio file:", error);
        showAudioFileWarning(audioPath);
      }
    } else {
      // File doesn't exist, show warning
      showAudioFileWarning(audioPath);
    }
  } catch (error) {
    // Error checking file existence, show warning
    showAudioFileWarning(audioPath);
  }
}

function showAudioFileWarning(audioPath: string) {
  showAudioWarning.value = true;
  missingAudioPath.value = audioPath;
  transcriptionStatus.value = "Transcript loaded successfully. Audio file not found.";
}

function selectCorrectAudioFile() {
  // Create a file input specifically for loading audio without processing
  const fileInput = document.createElement('input');
  fileInput.type = 'file';
  fileInput.accept = '.wav,.mp3,.m4a,.aac,.flac,.ogg';
  fileInput.style.display = 'none';
  
  fileInput.addEventListener('change', async (event) => {
    const target = event.target as HTMLInputElement;
    if (target.files && target.files[0]) {
      const selectedFile = target.files[0];
      audioFile.value = selectedFile;
      
      try {
        // Start loading indication
        isLoadingAudio.value = true;
        audioLoadingProgress.value = 0;
        showAudioWarning.value = false; // Hide warning, show progress instead
        
        // Use chunked upload for large files
        const chunkSize = 1024 * 1024; // 1MB chunks
        const totalChunks = Math.ceil(selectedFile.size / chunkSize);
        
        // Generate a unique session ID for this upload
        const sessionId = Date.now().toString() + Math.random().toString(36).substr(2, 9);
        
        let processedFilePath = "";
        
        for (let i = 0; i < totalChunks; i++) {
          const start = i * chunkSize;
          const end = Math.min(start + chunkSize, selectedFile.size);
          const chunk = selectedFile.slice(start, end);
          
          const chunkArray = new Uint8Array(await chunk.arrayBuffer());
          const result = await invoke('save_audio_file_chunked', {
            chunkData: Array.from(chunkArray),
            chunkIndex: i,
            totalChunks: totalChunks,
            filename: selectedFile.name,
            sessionId: sessionId
          });
          
          // The last chunk returns the final processed file path
          if (i === totalChunks - 1) {
            processedFilePath = result as string;
          }
          
          // Update progress (upload is 80% of total progress)
          const uploadProgress = ((i + 1) / totalChunks) * 80;
          audioLoadingProgress.value = uploadProgress;
          transcriptionStatus.value = `Uploading audio: ${Math.round(uploadProgress)}%`;
        }
        
        // Convert the saved file to base64 for playback (remaining 20%)
        audioLoadingProgress.value = 85;
        transcriptionStatus.value = "Processing audio for playback...";
        
        const result = await invoke('convert_audio_to_base64', {
          filePath: processedFilePath,
        });
        
        if (typeof result === 'string') {
          audioLoadingProgress.value = 100;
          originalAudioBase64.value = result;
          transcriptionStatus.value = "Audio loaded successfully. Ready for playback.";
          
          // Create audio player
          createAudioPlayer();
        }
      } catch (error) {
        console.error("Error loading audio:", error);
        errorMsg.value = `Error loading audio file: ${error}`;
        // Show warning again if loading failed
        showAudioWarning.value = true;
      } finally {
        // Hide loading indication
        isLoadingAudio.value = false;
        audioLoadingProgress.value = 0;
      }
    }
    
    // Clean up the temporary input
    document.body.removeChild(fileInput);
  });
  
  // Add to DOM and trigger click
  document.body.appendChild(fileInput);
  fileInput.click();
}

function parseNoscribeTranscript(bodyElement: HTMLElement): any[] {
  const segments: any[] = [];
  const tempSegments: any[] = [];
  
  // Look for noscribe-specific structure: <p><a name="...">Speaker: [timestamp] text</a></p>
  const paragraphs = Array.from(bodyElement.querySelectorAll('p')).filter(p => p.querySelector('a'));
  
  // First pass: collect all segments with start times
  paragraphs.forEach((p) => {
    const anchorElement = p.querySelector('a');
    if (!anchorElement) return;
    
    const fullText = anchorElement.textContent?.trim();
    if (!fullText) return;
    
    // Parse the noscribe format: "S03: [00:00:14] Ja, gerne. Wir können gerne ausführlich werden und ausschweifend."
    const timestampMatch = fullText.match(/\[(\d{2}):(\d{2}):(\d{2})\]/);
    if (!timestampMatch) {
      console.warn("No valid timestamp found in:", fullText);
      return; // Skip this segment if no timestamp found
    }
    
    const hours = parseInt(timestampMatch[1]);
    const minutes = parseInt(timestampMatch[2]);
    const seconds = parseInt(timestampMatch[3]);
    const startTimeSeconds = hours * 3600 + minutes * 60 + seconds;
    
    // Extract speaker and text
    const speakerMatch = fullText.match(/^([^:]+):/);
    const speaker = speakerMatch ? speakerMatch[1].trim() : '';
    
    // Extract the actual transcription text (everything after the timestamp)
    const textAfterTimestamp = fullText.substring(fullText.indexOf(']') + 1).trim();
    const transcriptionText = textAfterTimestamp || fullText;
    
    tempSegments.push({
      start_time_seconds: startTimeSeconds,
      transcription: `${speaker ? speaker + ': ' : ''}${transcriptionText}`,
    });
  });
  
  // Second pass: calculate end times using next segment's start time
  tempSegments.forEach((segment, index) => {
    let endTimeSeconds;
    
    if (index < tempSegments.length - 1) {
      // Use the start time of the next segment as the end time
      endTimeSeconds = tempSegments[index + 1].start_time_seconds;
    } else {
      // For the last segment, estimate duration based on text length
      const wordCount = segment.transcription.split(/\s+/).length;
      const estimatedDuration = Math.max(wordCount / 3, 2); // At least 2 seconds
      endTimeSeconds = segment.start_time_seconds + estimatedDuration;
    }
    
    segments.push({
      start_sample: Math.floor(segment.start_time_seconds * 16000), // Assuming 16kHz sample rate
      end_sample: Math.floor(endTimeSeconds * 16000),
      start_time_seconds: segment.start_time_seconds,
      end_time_seconds: endTimeSeconds,
      audio_data: [],
      audio_base64: '', // No audio data in imported file
      transcription: segment.transcription,
      isTranscribing: false,
      transcriptionError: null
    });
  });
  
  // If no properly formatted segments found, fall back to basic parsing
  if (segments.length === 0) {
    paragraphs.forEach((p, index) => {
      const text = p.textContent?.trim();
      if (text && text.length > 0) {
        const estimatedDuration = Math.max(text.length / 100, 3);
        const startTime = index * estimatedDuration;
        const endTime = startTime + estimatedDuration;
        
        segments.push({
          start_sample: Math.floor(startTime * 16000),
          end_sample: Math.floor(endTime * 16000),
          start_time_seconds: startTime,
          end_time_seconds: endTime,
          audio_data: [],
          audio_base64: '',
          transcription: text,
          isTranscribing: false,
          transcriptionError: null
        });
      }
    });
  }
  
  return segments;
}

async function handleFileSelect(event: Event) {
  const target = event.target as HTMLInputElement;
  if (target.files && target.files[0]) {
    audioFile.value = target.files[0];
    vadResults.value = [];
    errorMsg.value = "";
    originalAudioBase64.value = "";
    
    // Set project title to file name without extension
    const fileName = target.files[0].name;
    const nameWithoutExtension = fileName.substring(0, fileName.lastIndexOf('.')) || fileName;
    projectName.value = nameWithoutExtension;
    
    // Immediately show processing indicator
    isProcessing.value = true;
    transcriptionStatus.value = `Loading ${fileName}...`;
    
    await processAudio();
  }
}

async function processAudio() {
  if (!audioFile.value) {
    errorMsg.value = "Please select an audio file first";
    return;
  }

  isProcessing.value = true;
  errorMsg.value = "";
  vadResults.value = [];
  originalAudioBase64.value = "";
  processingProgress.value = 0;
  processingStep.value = "Preparing audio file...";

  try {
    // Use chunked upload to avoid JavaScript memory limits with large files
    processingStep.value = "Preparing audio file...";
    processingProgress.value = 2;
    
    // Generate a unique session ID for this upload
    const sessionId = Date.now().toString() + Math.random().toString(36).substr(2, 9);
    
    // Read file in chunks to avoid memory issues
    const file = audioFile.value;
    const chunkSize = 1024 * 1024; // 1MB chunks
    const totalChunks = Math.ceil(file.size / chunkSize);
    
    processingStep.value = `Uploading file (0/${totalChunks} chunks)...`;
    processingProgress.value = 5;
    
    let tempFilePath = "";
    
    for (let chunkIndex = 0; chunkIndex < totalChunks; chunkIndex++) {
      const start = chunkIndex * chunkSize;
      const end = Math.min(start + chunkSize, file.size);
      const chunk = file.slice(start, end);
      const chunkArrayBuffer = await chunk.arrayBuffer();
      const chunkBytes = Array.from(new Uint8Array(chunkArrayBuffer));
      
      processingStep.value = `Uploading file (${chunkIndex + 1}/${totalChunks} chunks)...`;
      processingProgress.value = 5 + (chunkIndex / totalChunks) * 3; // 5-8% for upload
      
      const result = await invoke("save_audio_file_chunked", {
        chunkData: chunkBytes,
        chunkIndex: chunkIndex,
        totalChunks: totalChunks,
        filename: file.name,
        sessionId: sessionId
      });
      
      // The last chunk returns the final file path
      if (chunkIndex === totalChunks - 1) {
        tempFilePath = result as string;
      }
    }
    
    processingStep.value = "Preparing audio for playback...";
    processingProgress.value = 8;
    
    // Convert audio to base64 in the backend (handles large files better)
    originalAudioBase64.value = await invoke("convert_audio_to_base64", { 
      filePath: tempFilePath 
    }) as string;
    
    createAudioPlayer();
    
    // Backend processing with real progress events
    processingStep.value = "Initializing audio processing...";
    processingProgress.value = 10;
    
    // The backend will emit progress events that automatically update our progress
    const segments = await invoke("process_audio_vad", { filePath: tempFilePath });
    vadResults.value = segments as any[];
    
    transcriptionStatus.value = `Found ${vadResults.value.length} speech segments. Ready for transcription.`;
    
    // Automatically start transcription after VAD processing
    if (vadResults.value.length > 0) {
      transcribeAllSegments();
    } else {
      transcriptionStatus.value = "No speech segments detected in the audio file.";
    }
  } catch (error) {
    errorMsg.value = `Error processing audio: ${error}`;
    transcriptionStatus.value = "";
    console.error("Audio processing error:", error);
  } finally {
    isProcessing.value = false;
  }
}

function formatTime(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = (seconds % 60).toFixed(2);
  return `${mins}:${secs.padStart(5, '0')}`;
}

function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 Bytes';
  const k = 1024;
  const sizes = ['Bytes', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

function updateHighlightedSegment() {
  if (!vadResults.value.length || !currentAudio.value) return;
  
  const currentTimeValue = currentAudio.value.currentTime;
  
  // Find the segment that contains the current time
  let foundIndex = -1;
  for (let i = 0; i < vadResults.value.length; i++) {
    const segment = vadResults.value[i];
    if (currentTimeValue >= segment.start_time_seconds && currentTimeValue <= segment.end_time_seconds) {
      foundIndex = i;
      break;
    }
  }
  
  currentHighlightedSegment.value = foundIndex;
}

function createAudioPlayer() {
  if (!originalAudioBase64.value) return;
  
  // Stop any currently playing audio
  if (currentAudio.value) {
    currentAudio.value.pause();
    currentAudio.value = null;
  }
  
  // Determine the MIME type based on the original file
  let mimeType = 'audio/wav'; // default
  if (audioFile.value) {
    const fileName = audioFile.value.name.toLowerCase();
    if (fileName.endsWith('.mp3')) mimeType = 'audio/mpeg';
    else if (fileName.endsWith('.m4a') || fileName.endsWith('.aac')) mimeType = 'audio/aac';
    else if (fileName.endsWith('.flac')) mimeType = 'audio/flac';
    else if (fileName.endsWith('.ogg')) mimeType = 'audio/ogg';
  }
  
  // Create new audio element with the original audio file
  const audio = new Audio(`data:${mimeType};base64,${originalAudioBase64.value}`);
  currentAudio.value = audio;
  
  // Set up event listeners
  audio.addEventListener('loadedmetadata', () => {
    duration.value = audio.duration;
    currentTime.value = audio.currentTime;
  });
  
  audio.addEventListener('timeupdate', () => {
    currentTime.value = audio.currentTime;
    updateHighlightedSegment();
  });
  
  audio.addEventListener('play', () => {
    isPlaying.value = true;
  });
  
  audio.addEventListener('pause', () => {
    isPlaying.value = false;
  });
  
  audio.addEventListener('ended', () => {
    isPlaying.value = false;
    currentTime.value = 0;
  });
  
  audio.addEventListener('error', () => {
    console.error("Error loading audio");
    currentAudio.value = null;
    showAudioPlayer.value = false;
  });
  
  // Show the audio player
  showAudioPlayer.value = true;
}

function playSegment(segment: any) {
  if (!currentAudio.value) {
    createAudioPlayer();
    if (!currentAudio.value) return;
  }
  
  // Jump to the segment start time
  currentAudio.value.currentTime = Math.max(segment.start_time_seconds - 1, 0);
  
  // Play the audio
  currentAudio.value.play().catch(error => {
    console.error("Error playing audio:", error);
  });
}

function togglePlayPause() {
  if (!currentAudio.value) {
    createAudioPlayer();
    if (!currentAudio.value) return;
  }
  
  if (isPlaying.value) {
    currentAudio.value.pause();
  } else {
    currentAudio.value.play().catch(error => {
      console.error("Error playing audio:", error);
    });
  }
}

function seekToTime(time: number) {
  if (currentAudio.value) {
    currentAudio.value.currentTime = Math.max(0, Math.min(time, duration.value));
  }
}

function handleSliderChange(event: Event) {
  const target = event.target as HTMLInputElement;
  const newTime = parseFloat(target.value);
  seekToTime(newTime);
}

function jumpToPreviousSegment() {
  if (!vadResults.value.length || !currentAudio.value) return;
  
  const currentTimeValue = currentAudio.value.currentTime;
  let targetSegment = null;
  
  // Find the previous segment
  for (let i = vadResults.value.length - 1; i >= 0; i--) {
    const segment = vadResults.value[i];
    if (segment.start_time_seconds < currentTimeValue - 1) {
      targetSegment = segment;
      break;
    }
  }
  
  if (targetSegment) {
    currentAudio.value.currentTime = Math.max(targetSegment.start_time_seconds - 1, 0);
  } else {
    // Jump to beginning if no previous segment
    currentAudio.value.currentTime = 0;
  }
}

function jumpToNextSegment() {
  if (!vadResults.value.length || !currentAudio.value) return;
  
  const currentTimeValue = currentAudio.value.currentTime;
  let targetSegment = null;
  
  // Find the next segment
  for (let i = 0; i < vadResults.value.length; i++) {
    const segment = vadResults.value[i];
    if (segment.start_time_seconds > currentTimeValue + 1) {
      targetSegment = segment;
      break;
    }
  }
  
  if (targetSegment) {
    currentAudio.value.currentTime = Math.max(targetSegment.start_time_seconds - 1, 0);
  } else {
    // Jump to end if no next segment
    currentAudio.value.currentTime = duration.value;
  }
}

function handleSegmentClick(event: MouseEvent, segment: any) {
  // Only play audio if Ctrl key is held down
  if (event.ctrlKey) {
    event.preventDefault();
    playSegment(segment);
  }
  // Otherwise, let the contenteditable functionality work normally
}

function handleSegmentEdit(event: Event, segment: any, index: number) {
  const target = event.target as HTMLElement;
  segment.transcription = target.textContent || '';
}

async function transcribeSegment(segment: any, index: number) {

  // Mark this segment as transcribing
  segment.isTranscribing = true;
  segment.transcriptionError = null;

  try {
    let audioBase64ToUse = segment.audio_base64;
    
    // If the segment doesn't have audio_base64 (e.g., from noscribe import),
    // we need to extract it from the original audio file
    if (!audioBase64ToUse && originalAudioBase64.value) {
      try {
        // Use the Rust backend to extract the segment audio from the original file
        audioBase64ToUse = await invoke("extract_segment_audio", {
          originalAudioBase64: originalAudioBase64.value,
          startTimeSeconds: segment.start_time_seconds,
          endTimeSeconds: segment.end_time_seconds
        });
      } catch (extractError) {
        console.error("Error extracting segment audio:", extractError);
        segment.transcriptionError = `Error extracting audio: ${extractError}`;
        return;
      }
    }
    
    // Check if we have audio data to transcribe
    if (!audioBase64ToUse) {
      segment.transcriptionError = "No audio data available for this segment";
      return;
    }

    // Call the Rust backend for transcription
    const transcription = await invoke("transcribe_audio", { 
      audioBase64: audioBase64ToUse, 
      segmentIndex: index,
      apiKey: apiKey.value,
      baseUrl: baseUrl.value,
      modelName: modelName.value
    });
    
    segment.transcription = transcription as string;
    
  } catch (error) {
    console.error("Transcription error:", error);
    segment.transcriptionError = `Error: ${error}`;
  } finally {
    segment.isTranscribing = false;
  }
}

async function transcribeAllSegments() {
  if (vadResults.value.length === 0) {
    errorMsg.value = "No segments to transcribe. Please process an audio file first.";
    return;
  }

  isTranscribing.value = true;
  transcriptionStatus.value = "Starting transcription...";
  errorMsg.value = "";

  let completed = 0;
  const total = vadResults.value.length;

  try {
    // Transcribe segments sequentially to avoid rate limiting
    for (let i = 0; i < vadResults.value.length; i++) {
      const segment = vadResults.value[i];
      transcriptionStatus.value = `Transcribing segment ${i + 1} of ${total}... (${Math.round((completed / total) * 100)}%)`;
      
      await transcribeSegment(segment, i);
      completed++;
      
      // Add a small delay to avoid rate limiting
      if (i < vadResults.value.length - 1) {
        await new Promise(resolve => setTimeout(resolve, 500));
      }
    }
    
    transcriptionStatus.value = `Completed transcription of ${completed} segments`;
  } catch (error) {
    errorMsg.value = `Transcription failed: ${error}`;
    transcriptionStatus.value = "";
  } finally {
    isTranscribing.value = false;
  }
}

// File operations
async function saveProject() {
  try {
    const projectData = {
      name: projectName.value,
      audioFileName: audioFile.value?.name || '',
      originalAudioBase64: originalAudioBase64.value,
      segments: vadResults.value.map(segment => ({
        start_sample: segment.start_sample,
        end_sample: segment.end_sample,
        start_time_seconds: segment.start_time_seconds,
        end_time_seconds: segment.end_time_seconds,
        transcription: segment.transcription || '',
        // Only keep audio_base64 if it exists (for backward compatibility)
        ...(segment.audio_base64 && { audio_base64: segment.audio_base64 })
      }))
    };
    
    const dataStr = JSON.stringify(projectData, null, 2);
    const blob = new Blob([dataStr], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    
    const a = document.createElement('a');
    a.href = url;
    a.download = `${projectName.value}.json`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  } catch (error) {
    errorMsg.value = `Failed to save project: ${error}`;
  }
}

async function loadProject(event: Event) {
  const target = event.target as HTMLInputElement;
  if (!target.files || !target.files[0]) return;
  
  try {
    const file = target.files[0];
    const text = await file.text();
    const projectData = JSON.parse(text);
    
    projectName.value = projectData.name || 'Loaded Project';
    
    // Restore original audio if available
    originalAudioBase64.value = projectData.originalAudioBase64 || '';
    
    // Restore segments with all their data
    vadResults.value = (projectData.segments || []).map((segment: any) => ({
      start_sample: segment.start_sample || 0,
      end_sample: segment.end_sample || 0,
      start_time_seconds: segment.start_time_seconds || 0,
      end_time_seconds: segment.end_time_seconds || 0,
      transcription: segment.transcription || '',
      // Keep audio_base64 for backward compatibility with old projects
      audio_base64: segment.audio_base64 || '',
      audio_data: segment.audio_data || [],
      isTranscribing: false,
      transcriptionError: null
    }));
    
    // If we have the original audio but no filename was saved, create a mock file reference
    if (originalAudioBase64.value && !audioFile.value) {
      // Create a filename based on the project name
      const audioFileName = projectData.audioFileName || `${projectData.name || 'audio'}.wav`;
      // We can't recreate the File object, but we can store the filename for display
      // audioFile.value will remain null, but we have originalAudioBase64.value for playback
    }
    
    // Clear the file input
    target.value = '';
    
    // Create audio player if we have original audio data
    if (originalAudioBase64.value) {
      createAudioPlayer();
    }
    
    // Show success message
    transcriptionStatus.value = `Loaded project with ${vadResults.value.length} segments`;
  } catch (error) {
    errorMsg.value = `Failed to load project: ${error}`;
  }
}

function exportAsText() {
  if (vadResults.value.length === 0) {
    errorMsg.value = "No transcription to export";
    return;
  }
  
  const text = vadResults.value
    .filter(segment => segment.transcription)
    .map(segment => segment.transcription)
    .join(' ');
  
  downloadFile(text, `${projectName.value}.txt`, 'text/plain');
}

function exportAsMarkdown() {
  if (vadResults.value.length === 0) {
    errorMsg.value = "No transcription to export";
    return;
  }
  
  let markdown = `# ${projectName.value}\n\n`;
  
  vadResults.value.forEach((segment, index) => {
    if (segment.transcription) {
      const startTime = formatTime(segment.start_time_seconds);
      const endTime = formatTime(segment.end_time_seconds);
      markdown += `## Segment ${index + 1} (${startTime} - ${endTime})\n\n`;
      markdown += `${segment.transcription}\n\n`;
    }
  });
  
  downloadFile(markdown, `${projectName.value}.md`, 'text/markdown');
}

function exportAsVTT() {
  if (vadResults.value.length === 0) {
    errorMsg.value = "No transcription to export";
    return;
  }
  
  let vtt = 'WEBVTT\n\n';
  
  vadResults.value.forEach((segment, index) => {
    if (segment.transcription) {
      const startTime = formatTimeVTT(segment.start_time_seconds);
      const endTime = formatTimeVTT(segment.end_time_seconds);
      vtt += `${index + 1}\n`;
      vtt += `${startTime} --> ${endTime}\n`;
      vtt += `${segment.transcription}\n\n`;
    }
  });
  
  downloadFile(vtt, `${projectName.value}.vtt`, 'text/vtt');
}

function exportAsSRT() {
  if (vadResults.value.length === 0) {
    errorMsg.value = "No transcription to export";
    return;
  }
  
  let srt = '';
  let segmentIndex = 1;
  
  vadResults.value.forEach((segment) => {
    if (segment.transcription) {
      const startTime = formatTimeSRT(segment.start_time_seconds);
      const endTime = formatTimeSRT(segment.end_time_seconds);
      srt += `${segmentIndex}\n`;
      srt += `${startTime} --> ${endTime}\n`;
      srt += `${segment.transcription}\n\n`;
      segmentIndex++;
    }
  });
  
  downloadFile(srt, `${projectName.value}.srt`, 'text/srt');
}

function downloadFile(content: string, filename: string, mimeType: string) {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

function formatTimeVTT(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  const secs = (seconds % 60).toFixed(3);
  return `${hours.toString().padStart(2, '0')}:${mins.toString().padStart(2, '0')}:${secs.padStart(6, '0')}`;
}

function formatTimeSRT(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);
  const ms = Math.floor((seconds % 1) * 1000);
  return `${hours.toString().padStart(2, '0')}:${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')},${ms.toString().padStart(3, '0')}`;
}

// Menu functions
function toggleFileMenu() {
  showFileMenu.value = !showFileMenu.value;
  showEditMenu.value = false;
  showHelpMenu.value = false;
}

function toggleEditMenu() {
  showEditMenu.value = !showEditMenu.value;
  showFileMenu.value = false;
  showHelpMenu.value = false;
}

function toggleHelpMenu() {
  showHelpMenu.value = !showHelpMenu.value;
  showFileMenu.value = false;
  showEditMenu.value = false;
}

function closeAllMenus() {
  showFileMenu.value = false;
  showEditMenu.value = false;
  showHelpMenu.value = false;
}

function newProject() {
  vadResults.value = [];
  audioFile.value = null;
  originalAudioBase64.value = "";
  projectName.value = "Untitled Project";
  errorMsg.value = "";
  transcriptionStatus.value = "";
  showAudioPlayer.value = false;
  currentSegmentInfo.value = "";
  currentTime.value = 0;
  duration.value = 0;
  isPlaying.value = false;
  currentHighlightedSegment.value = -1;
  processingProgress.value = 0;
  processingStep.value = "";
  if (currentAudio.value) {
    currentAudio.value.pause();
    currentAudio.value = null;
  }
  closeAllMenus();
}

function openAudioFile() {
  document.getElementById('audio-file')?.click();
  closeAllMenus();
}

function openNoscribeFile() {
  document.getElementById('noscribe-file')?.click();
  closeAllMenus();
}

function openProject() {
  document.getElementById('load-project')?.click();
  closeAllMenus();
}

function saveProjectFromMenu() {
  saveProject();
  closeAllMenus();
}

function selectAll() {
  // Select all transcribed text
  const selection = window.getSelection();
  const range = document.createRange();
  const transcriptionContainer = document.querySelector('.transcription-text');
  if (transcriptionContainer && selection) {
    range.selectNodeContents(transcriptionContainer);
    selection.removeAllRanges();
    selection.addRange(range);
  }
  closeAllMenus();
}

function copyAll() {
  const text = vadResults.value
    .filter(segment => segment.transcription)
    .map(segment => segment.transcription)
    .join(' ');
  
  navigator.clipboard.writeText(text).then(() => {
    transcriptionStatus.value = "Text copied to clipboard";
    setTimeout(() => {
      if (transcriptionStatus.value === "Text copied to clipboard") {
        transcriptionStatus.value = "";
      }
    }, 2000);
  });
  closeAllMenus();
}
</script>

<template>
  <main class="app-container" @click="closeAllMenus">
    <!-- Application Toolbar -->
    <div class="toolbar">
      <div class="toolbar-left">
        <div class="menu-item" @click.stop="toggleFileMenu">
          File
          <div v-if="showFileMenu" class="dropdown-menu">
            <div class="menu-option" @click="newProject">
              <span class="menu-icon">📄</span>
              <span>New Project</span>
              <span class="menu-shortcut">Ctrl+N</span>
            </div>
            <div class="menu-option" @click="openProject">
              <span class="menu-icon">📁</span>
              <span>Open Project...</span>
              <span class="menu-shortcut">Ctrl+O</span>
            </div>
            <div class="menu-option" @click="openAudioFile">
              <span class="menu-icon">♪</span>
              <span>Open Audio File...</span>
            </div>
            <div class="menu-option" @click="openNoscribeFile">
              <span class="menu-icon">📋</span>
              <span>Import Noscribe HTML...</span>
            </div>
            <div class="menu-separator"></div>
            <div class="menu-option" @click="saveProjectFromMenu" :class="{ disabled: vadResults.length === 0 }">
              <span class="menu-icon">💾</span>
              <span>Save Project</span>
              <span class="menu-shortcut">Ctrl+S</span>
            </div>
            <div class="menu-separator"></div>
            <div class="menu-submenu" v-if="vadResults.length > 0">
              <span class="menu-icon">↗</span>
              <span>Export</span>
              <span class="menu-arrow">▶</span>
              <div class="submenu">
                <div class="menu-option" @click="exportAsText">
                  <span class="menu-icon">📄</span>
                  <span>Text (.txt)</span>
                  <span class="menu-shortcut">Ctrl+E</span>
                </div>
                <div class="menu-option" @click="exportAsMarkdown">
                  <span class="menu-icon">⬇</span>
                  <span>Markdown (.md)</span>
                  <span class="menu-shortcut">Ctrl+Shift+E</span>
                </div>
                <div class="menu-option" @click="exportAsVTT">
                  <span class="menu-icon">🎬</span>
                  <span>WebVTT (.vtt)</span>
                </div>
                <div class="menu-option" @click="exportAsSRT">
                  <span class="menu-icon">📺</span>
                  <span>SubRip (.srt)</span>
                </div>
              </div>
            </div>
          </div>
        </div>
        
        <div class="menu-item" @click.stop="toggleEditMenu">
          Edit
          <div v-if="showEditMenu" class="dropdown-menu">
            <div class="menu-option" @click="selectAll" :class="{ disabled: vadResults.length === 0 }">
              <span class="menu-icon">⬜</span>
              <span>Select All</span>
              <span class="menu-shortcut">Ctrl+A</span>
            </div>
            <div class="menu-option" @click="copyAll" :class="{ disabled: vadResults.length === 0 }">
              <span class="menu-icon">📋</span>
              <span>Copy All Text</span>
              <span class="menu-shortcut">Ctrl+C</span>
            </div>
            <div class="menu-separator"></div>
            <div class="menu-option" @click="showSettings = true">
              <span class="menu-icon">⚙️</span>
              <span>Settings</span>
            </div>
          </div>
        </div>
        
        <div class="menu-item" @click.stop="toggleHelpMenu">
          Help
          <div v-if="showHelpMenu" class="dropdown-menu">
            <div class="menu-option disabled">
              <span class="menu-icon">ℹ</span>
              <span>About Transcriber</span>
            </div>
            <div class="menu-separator"></div>
            <div class="menu-info">
              <strong>Keyboard Shortcuts:</strong><br>
              <small>Ctrl+S: Save Project</small><br>
              <small>Ctrl+O: Open Project</small><br>
              <small>Ctrl+E: Export Text</small><br>
              <small>Ctrl+Shift+E: Export Markdown</small><br>
              <small>Ctrl+Click: Play Audio Segment</small>
            </div>
          </div>
        </div>
      </div>
      
      <div class="toolbar-center">
        <input 
          v-model="projectName" 
          class="project-title"
          placeholder="Untitled Project"
        />
      </div>
      
      <div class="toolbar-right">
        <div class="status-indicator" v-if="isProcessing">
          <span class="spinner">⌛</span>
          Processing...
        </div>
        <div class="status-indicator" v-else-if="isTranscribing">
          <span class="spinner">⌛</span>
          Transcribing...
        </div>
      </div>
    </div>

    <!-- Hidden file inputs -->
    <input 
      type="file" 
      id="load-project" 
      accept=".json"
      @change="loadProject"
      style="display: none;"
    />
    <input 
      type="file" 
      accept=".wav,.mp3,.m4a,.aac,.flac,.ogg"
      @change="handleFileSelect"
      id="audio-file"
      style="display: none;"
    />
    <input 
      type="file" 
      accept=".html,.htm"
      @change="importNoscribeFile"
      id="noscribe-file"
      style="display: none;"
    />

    <!-- Main Content Area -->
    <div class="main-content">
      <!-- Audio File Warning -->
      <div v-if="showAudioWarning && !isLoadingAudio" class="warning-banner">
        <div class="warning-content">
          <div class="warning-icon">⚠️</div>
          <div class="warning-text">
            <strong>Audio file not found:</strong> {{ missingAudioPath }}
            <br>
            <small>The imported transcript references an audio file that could not be loaded.</small>
          </div>
          <button @click="selectCorrectAudioFile" class="warning-button">
            Select Audio File
          </button>
          <button @click="showAudioWarning = false" class="warning-dismiss">
            ×
          </button>
        </div>
      </div>

      <!-- Audio Loading Progress -->
      <div v-if="isLoadingAudio" class="progress-banner">
        <div class="progress-content">
          <div class="progress-icon">⌛</div>
          <div class="progress-text">
            <strong>Loading Audio File</strong>
            <br>
            <small>{{ transcriptionStatus }}</small>
          </div>
          <div class="progress-section">
            <div class="progress-bar-container">
              <div class="progress-bar-track">
                <div 
                  class="progress-bar-fill" 
                  :style="{ width: `${audioLoadingProgress}%` }"
                ></div>
              </div>
            </div>
            <span class="progress-percentage">{{ Math.round(audioLoadingProgress) }}%</span>
          </div>
        </div>
      </div>
      <!-- Processing Indicator -->
      <div v-if="isProcessing" class="processing-section">
        <div class="processing-card">
          <div class="processing-header">
            <div class="processing-spinner">⌛</div>
            <h3>Processing Audio</h3>
          </div>
          
          <!-- Progress Bar -->
          <div class="progress-section">
            <div class="progress-info">
              <span class="progress-percentage">{{ processingProgress.toFixed(0) }}%</span>
            </div>
            <div class="progress-bar-container">
              <div class="progress-bar-track">
                <div 
                  class="progress-bar-fill" 
                  :style="{ width: `${processingProgress}%` }"
                ></div>
              </div>
            </div>
          </div>
          
          <!-- File Info -->
          <div class="file-info" v-if="audioFile">
            <div class="file-details">
              <span class="file-icon">♪</span>
              <div class="file-text">
                <div class="file-name">{{ audioFile.name }}</div>
                <div class="file-size">{{ formatFileSize(audioFile.size) }}</div>
              </div>
            </div>
          </div>
          
          <!-- Processing Steps -->
          <div class="processing-steps-list">
            <div 
              v-for="(step, index) in processingSteps" 
              :key="index"
              class="processing-step"
              :class="{
                'completed': processingProgress > (index * 100 / processingSteps.length),
                'active': step === processingStep,
                'pending': processingProgress <= (index * 100 / processingSteps.length)
              }"
            >
              <div class="step-indicator">
                <span v-if="processingProgress > ((index + 1) * 100 / processingSteps.length)" class="step-check">✓</span>
                <span v-else-if="step === processingStep" class="step-spinner">⌛</span>
                <span v-else class="step-number">{{ index + 1 }}</span>
              </div>
              <span class="step-text">{{ step }}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Audio Upload Section -->
      <div v-else-if="!vadResults.length" class="upload-section">
        <div class="upload-card">
          <h2>Get Started</h2>
          <p>Upload an audio file to begin transcription</p>
          <button @click="openAudioFile" class="primary-button">
            <span style="margin-right: 8px;">♪</span>
            Select Audio File
          </button>
          <p class="file-hint">Supports WAV, MP3, M4A, AAC, FLAC, and OGG formats</p>
          
          <div style="margin: 20px 0; text-align: center; color: #888;">OR</div>
          
          <button @click="openNoscribeFile" class="button secondary">
            <span style="margin-right: 8px;">📋</span>
            Import Noscribe HTML
          </button>
          <p class="file-hint">Import existing transcripts from noscribe HTML files</p>
        </div>
      </div>

      <div v-if="errorMsg" class="error">
        {{ errorMsg }}
      </div>

      <div v-if="vadResults.length > 0" class="results">
        <div class="transcription-editor">
          <div class="editor-controls">
            <span class="segment-count">{{ vadResults.filter(s => s.transcription).length }} segments</span>
            <span class="total-duration">Total: {{ formatTime(vadResults[vadResults.length - 1]?.end_time_seconds || 0) }}</span>
          </div>
          
          <div class="transcription-text">
            <div v-for="(segment, index) in vadResults" :key="index" class="segment-line">
              <div v-if="segment.isTranscribing" class="segment-container loading" :class="{ 'highlighted': currentHighlightedSegment === index }">
                <div class="segment-meta">
                  <span class="segment-number">{{ index + 1 }}</span>
                  <span class="segment-time">{{ formatTime(segment.start_time_seconds) }}</span>
                  <button 
                    class="retranscribe-button" 
                    disabled
                    title="Transcribing in progress...">
                    ⌛
                  </button>
                </div>
                <span class="transcribing-segment">
                  <span style="margin-right: 4px;">⌛</span>
                  Transcribing...
                </span>
              </div>
              <div v-else-if="segment.transcription != null" class="segment-container" :class="{ 'highlighted': currentHighlightedSegment === index }">
                <div class="segment-meta">
                  <span class="segment-number">{{ index + 1 }}</span>
                  <span class="segment-time">{{ formatTime(segment.start_time_seconds) }}</span>
                  <button 
                    class="retranscribe-button" 
                    @click="transcribeSegment(segment, index)"
                    :disabled="segment.isTranscribing || isTranscribing"
                    title="Retranscribe this segment">
                    🔄
                  </button>
                </div>
                <span 
                  class="transcribed-segment" 
                  :class="{ 'ctrl-pressed': isCtrlPressed }"
                  contenteditable="true"
                  @click="handleSegmentClick($event, segment)"
                  @input="handleSegmentEdit($event, segment, index)"
                  spellcheck="false"
                  title="CTRL+Click to play audio segment">{{ segment.transcription }}</span>
              </div>
              <div v-else-if="segment.transcriptionError" class="segment-container error" :class="{ 'highlighted': currentHighlightedSegment === index }">
                <div class="segment-meta">
                  <span class="segment-number">{{ index + 1 }}</span>
                  <span class="segment-time">{{ formatTime(segment.start_time_seconds) }}</span>
                  <button 
                    class="retranscribe-button" 
                    @click="transcribeSegment(segment, index)"
                    :disabled="segment.isTranscribing || isTranscribing"
                    title="Retry transcription">
                    🔄
                  </button>
                </div>
                <span class="error-segment" :title="`Error: ${segment.transcriptionError}`">
                  <span style="margin-right: 4px;">⚠</span>
                  Error: {{ segment.transcriptionError }}
                </span>
              </div>
              <div v-else class="segment-container processing" :class="{ 'highlighted': currentHighlightedSegment === index }">
                <div class="segment-meta">
                  <span class="segment-number">{{ index + 1 }}</span>
                  <span class="segment-time">{{ formatTime(segment.start_time_seconds) }}</span>
                </div>
                <span class="processing-segment">
                  Processing...
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
    
    <!-- Fixed Audio Player -->
    <div v-if="showAudioPlayer && originalAudioBase64" class="audio-player-container">
      <div class="audio-player">
        <div class="audio-info-section">
          <div class="audio-icon-wrapper">
            <div class="audio-icon">♪</div>
            <div class="audio-visualizer">
              <div class="bar" :class="{ active: isPlaying }"></div>
              <div class="bar" :class="{ active: isPlaying }"></div>
              <div class="bar" :class="{ active: isPlaying }"></div>
              <div class="bar" :class="{ active: isPlaying }"></div>
            </div>
          </div>
          <div class="audio-details">
            <div class="audio-title">{{ audioFile?.name || projectName }}</div>
            <div class="audio-subtitle">
              {{ currentHighlightedSegment >= 0 ? `Playing segment ${currentHighlightedSegment + 1}` : 'Ready to play' }}
            </div>
          </div>
        </div>
        
        <div class="audio-controls-section">
          <div class="main-controls">
            <button @click="jumpToPreviousSegment" class="control-button secondary" title="Previous segment">
              <span class="control-icon">⏮</span>
            </button>
            <button @click="togglePlayPause" class="control-button primary" title="Play/Pause">
              <span class="control-icon">{{ isPlaying ? '⏸' : '▶' }}</span>
            </button>
            <button @click="jumpToNextSegment" class="control-button secondary" title="Next segment">
              <span class="control-icon">⏭</span>
            </button>
          </div>
          
          <div class="progress-section">
            <span class="time-display">{{ formatTime(currentTime) }}</span>
            <div class="progress-container">
              <div class="progress-track">
                <div class="progress-fill" :style="{ width: `${(currentTime / duration) * 100}%` }"></div>
                <input 
                  type="range" 
                  min="0" 
                  :max="duration || 100" 
                  :value="currentTime" 
                  @input="handleSliderChange"
                  class="progress-slider"
                />
              </div>
            </div>
            <span class="time-display">{{ formatTime(duration) }}</span>
          </div>
        </div>
        
      </div>
    </div>
  </main>

  <!-- Settings Modal -->
  <div v-if="showSettings" class="modal-overlay" @click="showSettings = false">
    <div class="modal-content" @click.stop>
      <div class="modal-header">
        <h3>Settings</h3>
        <button class="close-button" @click="showSettings = false">×</button>
      </div>
      
      <div class="modal-body">
        <div class="setting-group">
          <label for="api-key">API Key:</label>
          <input 
            id="api-key" 
            v-model="apiKey" 
            type="password" 
            placeholder="Enter your API key"
            class="setting-input"
          />
        </div>
        
        <div class="setting-group">
          <label for="base-url">Base URL:</label>
          <input 
            id="base-url" 
            v-model="baseUrl" 
            type="url" 
            placeholder="Enter the API base URL"
            class="setting-input"
          />
        </div>
        
        <div class="setting-group">
          <label for="model-name">Model Name:</label>
          <input 
            id="model-name" 
            v-model="modelName" 
            type="text" 
            placeholder="Enter the model name"
            class="setting-input"
          />
        </div>
      </div>
      
      <div class="modal-footer">
        <button @click="resetSettings" class="button secondary">Reset to Defaults</button>
        <button @click="saveSettings" class="button primary">Save Settings</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.app-container {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', sans-serif;
  background: #f0f0f0;
  min-height: 100vh;
  display: flex;
  flex-direction: column;
}

/* Toolbar Styles */
.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  background: linear-gradient(to bottom, #ffffff, #f8f8f8);
  border-bottom: 1px solid #d0d0d0;
  padding: 8px 16px;
  box-shadow: 0 1px 3px rgba(0,0,0,0.1);
  position: relative;
  z-index: 1000;
}

.toolbar-left {
  display: flex;
  gap: 0;
}

.toolbar-center {
  flex: 1;
  display: flex;
  justify-content: center;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.project-title {
  padding: 6px 12px;
  border: 1px solid transparent;
  border-radius: 4px;
  font-size: 14px;
  background: transparent;
  text-align: center;
  min-width: 200px;
  color: #333;
}

.project-title:hover {
  background: rgba(255,255,255,0.8);
  border-color: #ddd;
}

.project-title:focus {
  outline: none;
  background: white;
  border-color: #0066cc;
  box-shadow: 0 0 0 2px rgba(0,102,204,0.2);
}

.menu-item {
  position: relative;
  padding: 8px 16px;
  cursor: pointer;
  font-size: 14px;
  color: #333;
  border-radius: 4px;
  transition: background-color 0.2s;
}

.menu-item:hover {
  background: rgba(0,0,0,0.05);
}

.dropdown-menu {
  position: absolute;
  top: 100%;
  left: 0;
  background: white;
  border: 1px solid #d0d0d0;
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.15);
  min-width: 200px;
  padding: 4px 0;
  z-index: 1001;
}

.menu-option {
  display: flex;
  align-items: center;
  padding: 8px 16px;
  cursor: pointer;
  font-size: 13px;
  color: #333;
  transition: background-color 0.2s;
  gap: 8px;
}

.menu-option:hover:not(.disabled) {
  background: #0066cc;
  color: white;
}

.menu-option.disabled {
  color: #999;
  cursor: not-allowed;
}

.menu-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  text-align: center;
}

.menu-icon svg {
  width: 14px;
  height: 14px;
}

.menu-shortcut {
  margin-left: auto;
  font-size: 11px;
  color: #666;
}

.menu-option:hover:not(.disabled) .menu-shortcut {
  color: rgba(255,255,255,0.8);
}

.menu-separator {
  height: 1px;
  background: #e0e0e0;
  margin: 4px 0;
}

.menu-submenu {
  position: relative;
  display: flex;
  align-items: center;
  padding: 8px 16px;
  cursor: pointer;
  font-size: 13px;
  color: #333;
  transition: background-color 0.2s;
  gap: 8px;
}

.menu-submenu:hover {
  background: #0066cc;
  color: white;
}

.menu-arrow {
  margin-left: auto;
  font-size: 10px;
}

.submenu {
  position: absolute;
  top: 0;
  left: 100%;
  background: white;
  border: 1px solid #d0d0d0;
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.15);
  min-width: 180px;
  padding: 4px 0;
  display: none;
}

.menu-submenu:hover .submenu {
  display: block;
}

.menu-info {
  padding: 8px 16px;
  font-size: 12px;
  color: #666;
  line-height: 1.4;
}

.status-indicator {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #666;
}

.spinner {
  display: flex;
  align-items: center;
  justify-content: center;
  animation: spin 1s linear infinite;
}

.spinner svg {
  width: 100%;
  height: 100%;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

@keyframes pulse {
  0% { 
    transform: scale(1);
    box-shadow: 0 0 0 0 rgba(33, 150, 243, 0.4);
  }
  70% {
    transform: scale(1.05);
    box-shadow: 0 0 0 8px rgba(33, 150, 243, 0);
  }
  100% {
    transform: scale(1);
    box-shadow: 0 0 0 0 rgba(33, 150, 243, 0);
  }
}

/* Main Content */
.main-content {
  flex: 1;
  padding: 20px;
  padding-bottom: 300px;
}

/* Warning Banner */
.warning-banner {
  background: #fff3cd;
  border: 1px solid #ffeaa7;
  border-radius: 8px;
  margin-bottom: 20px;
  box-shadow: 0 2px 8px rgba(0,0,0,0.1);
}

.warning-content {
  display: flex;
  align-items: center;
  padding: 16px;
  gap: 12px;
}

.warning-icon {
  font-size: 24px;
  flex-shrink: 0;
}

.warning-text {
  flex: 1;
  color: #856404;
  line-height: 1.4;
}

.warning-text strong {
  color: #533f03;
}

.warning-text small {
  color: #6c5f04;
}

.warning-button {
  background: #ffc107;
  color: #212529;
  border: none;
  padding: 8px 16px;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background-color 0.2s;
  flex-shrink: 0;
}

.warning-button:hover {
  background: #e0a800;
}

.warning-dismiss {
  background: none;
  border: none;
  font-size: 20px;
  color: #856404;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
  transition: background-color 0.2s;
  flex-shrink: 0;
}

.warning-dismiss:hover {
  background: rgba(0,0,0,0.1);
}

/* Progress Banner */
.progress-banner {
  background: #e3f2fd;
  border: 1px solid #90caf9;
  border-radius: 8px;
  margin-bottom: 20px;
  box-shadow: 0 2px 8px rgba(0,0,0,0.1);
}

.progress-content {
  display: flex;
  align-items: center;
  padding: 16px;
  gap: 12px;
}

.progress-icon {
  font-size: 24px;
  flex-shrink: 0;
  animation: spin 1s linear infinite;
}

.progress-text {
  flex: 1;
  color: #1565c0;
  line-height: 1.4;
}

.progress-text strong {
  color: #0d47a1;
}

.progress-text small {
  color: #1976d2;
}

.progress-section {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 200px;
  flex-shrink: 0;
}

.progress-percentage {
  font-size: 14px;
  font-weight: 600;
  color: #1976d2;
  min-width: 40px;
}

.processing-section {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 60vh;
}

.processing-card {
  background: white;
  padding: 32px;
  border-radius: 16px;
  box-shadow: 0 8px 32px rgba(0,0,0,0.1);
  text-align: left;
  max-width: 500px;
  border-left: 4px solid #2196f3;
  min-width: 450px;
}

.processing-header {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 24px;
}

.processing-spinner {
  font-size: 32px;
  animation: spin 1s linear infinite;
  flex-shrink: 0;
}

.processing-card h3 {
  margin: 0;
  color: #333;
  font-size: 24px;
  font-weight: 600;
}

.progress-section {
  margin-bottom: 24px;
}

.progress-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.progress-text {
  color: #666;
  font-size: 14px;
  font-weight: 500;
}

.progress-percentage {
  color: #2196f3;
  font-size: 14px;
  font-weight: 600;
}

.progress-bar-container {
  position: relative;
  width: 100%;
  height: 8px;
  background: #f0f0f0;
  border-radius: 4px;
  overflow: hidden;
}

.progress-bar-track {
  width: 100%;
  height: 100%;
  position: relative;
}

.progress-bar-fill {
  height: 100%;
  background: linear-gradient(90deg, #2196f3, #21cbf3);
  border-radius: 4px;
  transition: width 0.3s ease;
  position: relative;
  overflow: hidden;
}

.progress-bar-fill::after {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  bottom: 0;
  right: 0;
  background: linear-gradient(90deg, transparent, rgba(255,255,255,0.3), transparent);
  animation: shimmer 2s infinite;
}

@keyframes shimmer {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(100%); }
}

.file-info {
  background: #f8f9fa;
  padding: 16px;
  border-radius: 8px;
  margin-bottom: 24px;
}

.file-details {
  display: flex;
  align-items: center;
  gap: 12px;
}

.file-icon {
  font-size: 24px;
  color: #2196f3;
  flex-shrink: 0;
}

.file-text {
  flex: 1;
}

.file-name {
  font-weight: 500;
  color: #333;
  font-size: 14px;
  margin-bottom: 2px;
}

.file-size {
  color: #666;
  font-size: 12px;
}

.processing-steps-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.processing-step {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 0;
  transition: all 0.3s ease;
}

.processing-step.completed {
  color: #4caf50;
}

.processing-step.active {
  color: #2196f3;
  font-weight: 500;
}

.processing-step.pending {
  color: #999;
}

.step-indicator {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: bold;
  flex-shrink: 0;
  transition: all 0.3s ease;
}

.processing-step.completed .step-indicator {
  background: #4caf50;
  color: white;
}

.processing-step.active .step-indicator {
  background: #2196f3;
  color: white;
  animation: pulse 1.5s infinite;
}

.processing-step.pending .step-indicator {
  background: #e0e0e0;
  color: #999;
}

.step-text {
  font-size: 14px;
}

.step-spinner {
  animation: spin 1s linear infinite;
}

.upload-section {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 60vh;
}

.upload-card {
  background: white;
  padding: 40px;
  border-radius: 12px;
  box-shadow: 0 4px 20px rgba(0,0,0,0.1);
  text-align: center;
  max-width: 400px;
}

.upload-card h2 {
  margin: 0 0 16px 0;
  color: #333;
  font-size: 24px;
  font-weight: 600;
}

.upload-card p {
  margin: 0 0 24px 0;
  color: #666;
  font-size: 16px;
}

.primary-button {
  background: #0066cc;
  color: white;
  border: none;
  padding: 12px 24px;
  border-radius: 8px;
  font-size: 16px;
  cursor: pointer;
  transition: background-color 0.2s;
}

.primary-button:hover {
  background: #0052a3;
}

.file-hint {
  font-size: 14px !important;
  color: #999 !important;
  margin-top: 16px !important;
}

/* Existing transcription styles kept for compatibility */
.transcription-editor {
  border: 1px solid #e0e0e0;
  border-radius: 8px;
  background-color: #fff;
  margin-top: 20px;
}

.editor-controls {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.75rem 1rem;
  background-color: #f8f9fa;
  border-bottom: 1px solid #e0e0e0;
  font-size: 0.9rem;
  color: #666;
}

.segment-count, .total-duration {
  font-weight: 500;
}

.transcription-text {
  padding: 0;
}

.segment-line {
  border-bottom: 1px solid #f0f0f0;
}

.segment-line:last-child {
  border-bottom: none;
}

.segment-container {
  display: flex;
  align-items: flex-start;
  gap: 1rem;
  padding: 0.75rem 1rem;
  transition: background-color 0.2s ease;
}

.segment-container.highlighted {
  background-color: #e3f2fd !important;
  border-left: 4px solid #1976d2;
  animation: pulse 2s infinite;
}

@keyframes pulse {
  0% { 
    box-shadow: 0 0 0 0 rgba(25, 118, 210, 0.4);
  }
  70% {
    box-shadow: 0 0 0 8px rgba(25, 118, 210, 0);
  }
  100% {
    box-shadow: 0 0 0 0 rgba(25, 118, 210, 0);
  }
}

.segment-container:hover {
  background-color: #f8f9fa;
}

.segment-container.loading {
  background-color: #fff3cd;
}

.segment-container.error {
  background-color: #f8d7da;
}

.segment-container.processing {
  background-color: #d1ecf1;
}

.segment-meta {
  display: flex;
  flex-direction: column;
  align-items: center;
  min-width: 60px;
  flex-shrink: 0;
  gap: 0.25rem;
}

.segment-number {
  background-color: #4CAF50;
  color: white;
  border-radius: 50%;
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.8rem;
  font-weight: bold;
}

.segment-time {
  font-size: 0.75rem;
  color: #666;
  font-family: monospace;
}

.retranscribe-button {
  background: #e3f2fd;
  border: 1px solid #90caf9;
  border-radius: 4px;
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 10px;
  cursor: pointer;
  transition: all 0.2s ease;
  padding: 0;
  margin-top: 2px;
}

.retranscribe-button:hover:not(:disabled) {
  background: #bbdefb;
  border-color: #64b5f6;
  transform: scale(1.1);
}

.retranscribe-button:disabled {
  background: #f5f5f5;
  border-color: #e0e0e0;
  color: #bdbdbd;
  cursor: not-allowed;
  transform: none;
}

.transcribed-segment {
  flex: 1;
  cursor: text;
  transition: background-color 0.2s ease;
  padding: 0.5rem;
  border-radius: 4px;
  outline: none;
  border: 1px solid transparent;
  line-height: 1.6;
  min-height: 1.6em;
}

.transcribed-segment:hover {
  background-color: #f0f0f0;
  border: 1px solid #d0d0d0;
}

.transcribed-segment:focus {
  background-color: #fff;
  border: 1px solid #1976d2;
  box-shadow: 0 0 0 2px rgba(25, 118, 210, 0.1);
}

.transcribed-segment.ctrl-pressed {
  cursor: pointer !important;
}

.transcribed-segment.ctrl-pressed:hover {
  background-color: #bbdefb;
  color: #1976d2;
  text-decoration: underline;
}

.transcribing-segment, .processing-segment, .error-segment {
  flex: 1;
  padding: 0.5rem;
  font-style: italic;
}

.transcribing-segment, .processing-segment {
  color: #ff9800;
}

.error-segment {
  color: #f44336;
}

button:disabled {
  background-color: #cccccc;
  cursor: not-allowed;
}

/* Fixed Audio Player */
.audio-player-container {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  background: linear-gradient(135deg, rgba(15, 23, 42, 0.95), rgba(30, 41, 59, 0.95));
  backdrop-filter: blur(20px);
  z-index: 2000;
  padding: 16px 24px;
  box-shadow: 0 -8px 32px rgba(0, 0, 0, 0.4);
  border-top: 1px solid rgba(255, 255, 255, 0.1);
}

.audio-player {
  display: flex;
  align-items: center;
  gap: 24px;
  max-width: 1400px;
  margin: 0 auto;
  color: white;
}

.audio-info-section {
  display: flex;
  align-items: center;
  gap: 16px;
  min-width: 280px;
  flex-shrink: 0;
}

.audio-icon-wrapper {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 56px;
  height: 56px;
  background: linear-gradient(135deg, #3b82f6, #1d4ed8);
  border-radius: 16px;
  box-shadow: 0 4px 16px rgba(59, 130, 246, 0.3);
}

.audio-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24px;
  z-index: 1;
}

.audio-icon svg {
  width: 24px;
  height: 24px;
}

.audio-visualizer {
  position: absolute;
  bottom: 6px;
  right: 6px;
  display: flex;
  gap: 2px;
  align-items: end;
}

.audio-visualizer .bar {
  width: 2px;
  height: 4px;
  background: rgba(255, 255, 255, 0.4);
  border-radius: 1px;
  transition: all 0.3s ease;
}

.audio-visualizer .bar.active {
  animation: visualize 1.5s ease-in-out infinite;
}

.audio-visualizer .bar:nth-child(1).active { animation-delay: 0s; }
.audio-visualizer .bar:nth-child(2).active { animation-delay: 0.1s; }
.audio-visualizer .bar:nth-child(3).active { animation-delay: 0.2s; }
.audio-visualizer .bar:nth-child(4).active { animation-delay: 0.3s; }

@keyframes visualize {
  0%, 100% { 
    height: 4px; 
    background: rgba(255, 255, 255, 0.4);
  }
  50% { 
    height: 12px; 
    background: rgba(255, 255, 255, 0.9);
  }
}

.audio-details {
  flex: 1;
  min-width: 0;
  max-width: 300px; /* Ensure a maximum width for proper ellipsis */
}

.audio-title {
  font-weight: 600;
  font-size: 16px;
  color: white;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-bottom: 4px;
  line-height: 1.2;
  max-width: 100%; /* Ensure title respects parent width */
}

.audio-subtitle {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.7);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-weight: 400;
}

.audio-controls-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
}

.main-controls {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 16px;
}

.control-button {
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 12px;
  cursor: pointer;
  transition: all 0.2s ease;
  font-size: 16px;
  position: relative;
  overflow: hidden;
}

.control-button.primary {
  width: 56px;
  height: 56px;
  background: linear-gradient(135deg, #10b981, #059669);
  box-shadow: 0 4px 16px rgba(16, 185, 129, 0.3);
}

.control-button.primary:hover {
  transform: scale(1.05);
  box-shadow: 0 6px 20px rgba(16, 185, 129, 0.4);
}

.control-button.secondary {
  width: 44px;
  height: 44px;
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.2);
}

.control-button.secondary:hover {
  background: rgba(255, 255, 255, 0.2);
  border-color: rgba(255, 255, 255, 0.3);
  transform: translateY(-1px);
}

.control-button.tertiary {
  width: 40px;
  height: 40px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.control-button.tertiary:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: rgba(255, 255, 255, 0.2);
}

.control-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
  font-size: inherit;
}

.control-icon svg {
  width: 100%;
  height: 100%;
}

.progress-section {
  display: flex;
  align-items: center;
  gap: 16px;
}

.time-display {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.8);
  font-family: 'SF Mono', Consolas, 'Monaco', monospace;
  font-weight: 500;
  min-width: 50px;
  text-align: center;
}

.progress-container {
  flex: 1;
  min-width: 200px;
}

.progress-track {
  position: relative;
  height: 8px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #3b82f6, #1d4ed8);
  border-radius: 4px;
  transition: width 0.1s ease;
  position: relative;
}

.progress-fill::after {
  content: '';
  position: absolute;
  top: 0;
  right: 0;
  width: 3px;
  height: 100%;
  background: rgba(255, 255, 255, 0.8);
  border-radius: 0 4px 4px 0;
}

.progress-slider {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: transparent;
  border: none;
  outline: none;
  cursor: pointer;
  -webkit-appearance: none;
  appearance: none;
  z-index: 1;
}

.progress-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 20px;
  height: 20px;
  background: white;
  border-radius: 50%;
  cursor: pointer;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  border: 2px solid #3b82f6;
  opacity: 0;
  transition: opacity 0.2s ease;
}

.progress-slider:hover::-webkit-slider-thumb,
.progress-slider:active::-webkit-slider-thumb {
  opacity: 1;
}

.progress-slider::-moz-range-thumb {
  width: 20px;
  height: 20px;
  background: white;
  border-radius: 50%;
  cursor: pointer;
  border: 2px solid #3b82f6;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  opacity: 0;
  transition: opacity 0.2s ease;
}

.progress-slider:hover::-moz-range-thumb,
.progress-slider:active::-moz-range-thumb {
  opacity: 1;
}

.audio-actions {
  display: flex;
  align-items: center;
  gap: 16px;
  min-width: 120px;
  justify-content: flex-end;
}

.volume-indicator {
  display: flex;
  gap: 3px;
  align-items: end;
  height: 24px;
}

.volume-bar {
  width: 3px;
  height: 8px;
  background: rgba(255, 255, 255, 0.3);
  border-radius: 2px;
  transition: all 0.3s ease;
}

.volume-indicator.active .volume-bar {
  animation: volumePulse 1.2s ease-in-out infinite;
}

.volume-indicator.active .volume-bar:nth-child(1) { animation-delay: 0s; }
.volume-indicator.active .volume-bar:nth-child(2) { animation-delay: 0.15s; }
.volume-indicator.active .volume-bar:nth-child(3) { animation-delay: 0.3s; }

@keyframes volumePulse {
  0%, 100% { 
    height: 8px; 
    background: rgba(255, 255, 255, 0.3);
  }
  50% { 
    height: 18px; 
    background: rgba(255, 255, 255, 0.8);
  }
}

/* Responsive design */
@media (max-width: 768px) {
  .audio-player {
    flex-direction: column;
    gap: 16px;
  }
  
  .audio-info-section {
    min-width: auto;
    width: 100%;
    justify-content: center;
  }
  
  .audio-details {
    max-width: 100%;
    min-width: 0;
  }
  
  .audio-title {
    max-width: 250px; /* Smaller max width on mobile */
  }
  
  .audio-controls-section {
    width: 100%;
  }
  
  .audio-actions {
    min-width: auto;
    justify-content: center;
  }
  
  .progress-section {
    flex-direction: column;
    gap: 8px;
  }
  
  .time-display {
    order: -1;
  }
}

/* Focus styles for accessibility */
.control-button:focus {
  outline: 2px solid #3b82f6;
  outline-offset: 2px;
}

.progress-slider:focus {
  outline: none;
}

.progress-slider:focus + .progress-track {
  box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.5);
}
</style>
<style>
:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;

  color: #0f0f0f;
  background-color: #f6f6f6;

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

.container {
  margin: 0;
  padding-top: 10vh;
  display: flex;
  flex-direction: column;
  justify-content: center;
  text-align: center;
}

.logo {
  height: 6em;
  padding: 1.5em;
  will-change: filter;
  transition: 0.75s;
}

.logo.tauri:hover {
  filter: drop-shadow(0 0 2em #24c8db);
}

.row {
  display: flex;
  justify-content: center;
}

a {
  font-weight: 500;
  color: #646cff;
  text-decoration: inherit;
}

a:hover {
  color: #535bf2;
}

h1 {
  text-align: center;
}

input,
button {
  border-radius: 8px;
  border: 1px solid transparent;
  padding: 0.6em 1.2em;
  font-size: 1em;
  font-weight: 500;
  font-family: inherit;
  color: #0f0f0f;
  background-color: #ffffff;
  transition: border-color 0.25s;
  box-shadow: 0 2px 2px rgba(0, 0, 0, 0.2);
}

button {
  cursor: pointer;
}

button:hover {
  border-color: #396cd8;
}
button:active {
  border-color: #396cd8;
  background-color: #e8e8e8;
}

input,
button {
  outline: none;
}


@media (prefers-color-scheme: dark) {
  :root {
    color: #f6f6f6;
    background-color: #2f2f2f;
  }

  a:hover {
    color: #24c8db;
  }

  input,
  button {
    color: #ffffff;
    background-color: #0f0f0f98;
  }
  button:active {
    background-color: #0f0f0f69;
  }
}

body {
  margin: 0;
  overscroll-behavior: none;
}

/* Modal Styles */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 1000;
}

.modal-content {
  background: white;
  border-radius: 8px;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.2);
  max-width: 500px;
  width: 90%;
  max-height: 80vh;
  overflow-y: auto;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 20px 24px 16px;
  border-bottom: 1px solid #e0e0e0;
}

.modal-header h3 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
}

.close-button {
  background: none;
  border: none;
  font-size: 24px;
  cursor: pointer;
  padding: 0;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  color: #666;
}

.close-button:hover {
  background: #f0f0f0;
  color: #333;
}

.modal-body {
  padding: 24px;
}

.setting-group {
  margin-bottom: 20px;
}

.setting-group label {
  display: block;
  margin-bottom: 6px;
  font-weight: 500;
  color: #333;
}

.setting-input {
  width: 100%;
  padding: 10px 12px;
  border: 2px solid #e0e0e0;
  border-radius: 6px;
  font-size: 14px;
  transition: border-color 0.2s;
  box-sizing: border-box;
}

.setting-input:focus {
  border-color: #007AFF;
  outline: none;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  padding: 16px 24px 24px;
  border-top: 1px solid #e0e0e0;
}

.button {
  padding: 10px 20px;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.button.primary {
  background: #007AFF;
  color: white;
}

.button.primary:hover {
  background: #0056CC;
}

.button.secondary {
  background: #f0f0f0;
  color: #333;
}

.button.secondary:hover {
  background: #e0e0e0;
}

@media (prefers-color-scheme: dark) {
  .modal-content {
    background: #2d2d2d;
    color: #f6f6f6;
  }
  
  .modal-header {
    border-bottom-color: #444;
  }
  
  .modal-footer {
    border-top-color: #444;
  }
  
  .close-button {
    color: #ccc;
  }
  
  .close-button:hover {
    background: #444;
    color: #fff;
  }
  
  .setting-group label {
    color: #f6f6f6;
  }
  
  .setting-input {
    background: #3d3d3d;
    border-color: #555;
    color: #f6f6f6;
  }
  
  .setting-input:focus {
    border-color: #007AFF;
  }
  
  .button.secondary {
    background: #444;
    color: #f6f6f6;
  }
  
  .button.secondary:hover {
    background: #555;
  }
}

</style>