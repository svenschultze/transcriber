<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

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
});

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeyDown);
  document.removeEventListener('keyup', handleKeyUp);
});

async function handleFileSelect(event: Event) {
  const target = event.target as HTMLInputElement;
  if (target.files && target.files[0]) {
    audioFile.value = target.files[0];
    vadResults.value = [];
    errorMsg.value = "";
    
    // Immediately show processing indicator
    isProcessing.value = true;
    transcriptionStatus.value = `Loading ${target.files[0].name}...`;
    
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
  transcriptionStatus.value = "Preparing audio file...";

  try {
    // Convert file to array buffer, then to bytes
    transcriptionStatus.value = "Reading audio file...";
    const arrayBuffer = await audioFile.value.arrayBuffer();
    const bytes = Array.from(new Uint8Array(arrayBuffer));
    
    // Save file to temporary location
    transcriptionStatus.value = "Saving audio file...";
    const tempFilePath = await invoke("save_audio_file", { 
      fileData: bytes, 
      filename: audioFile.value.name 
    });
    
    // Process the audio file
    transcriptionStatus.value = "Analyzing audio with voice activity detection...";
    const segments = await invoke("process_audio_vad", { filePath: tempFilePath });
    vadResults.value = segments as any[];
    
    transcriptionStatus.value = `Found ${vadResults.value.length} speech segments. Starting transcription...`;
    
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

function playSegment(segment: any) {
  if (!segment.audio_base64) {
    console.error("No audio data available for this segment");
    return;
  }
  
  // Stop any currently playing audio
  if (currentAudio.value) {
    currentAudio.value.pause();
    currentAudio.value.currentTime = 0;
  }
  
  // Create new audio element and play the segment
  const audio = new Audio(`data:audio/wav;base64,${segment.audio_base64}`);
  currentAudio.value = audio;
  
  // Clear the reference when audio ends
  audio.addEventListener('ended', () => {
    if (currentAudio.value === audio) {
      currentAudio.value = null;
    }
  });
  
  // Clear the reference if audio fails to play
  audio.addEventListener('error', () => {
    if (currentAudio.value === audio) {
      currentAudio.value = null;
    }
  });
  
  audio.play().catch(error => {
    console.error("Error playing audio:", error);
    if (currentAudio.value === audio) {
      currentAudio.value = null;
    }
  });
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
  if (!segment.audio_base64) {
    errorMsg.value = "No audio data available for this segment";
    return;
  }

  // Mark this segment as transcribing
  segment.isTranscribing = true;
  segment.transcriptionError = null;

  try {
    // Call the Rust backend for transcription
    const transcription = await invoke("transcribe_audio", { 
      audioBase64: segment.audio_base64, 
      segmentIndex: index 
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
  transcriptionStatus.value = "Transcribing segments...";
  errorMsg.value = "";

  let completed = 0;
  const total = vadResults.value.length;

  try {
    // Transcribe segments sequentially to avoid rate limiting
    for (let i = 0; i < vadResults.value.length; i++) {
      const segment = vadResults.value[i];
      transcriptionStatus.value = `Transcribing segment ${i + 1} of ${total}...`;
      
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
      segments: vadResults.value.map(segment => ({
        start_sample: segment.start_sample,
        end_sample: segment.end_sample,
        start_time_seconds: segment.start_time_seconds,
        end_time_seconds: segment.end_time_seconds,
        transcription: segment.transcription || '',
        audio_base64: segment.audio_base64 || '',
        audio_data: segment.audio_data || []
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
    
    // Restore segments with all their data
    vadResults.value = (projectData.segments || []).map((segment: any) => ({
      start_sample: segment.start_sample || 0,
      end_sample: segment.end_sample || 0,
      start_time_seconds: segment.start_time_seconds || 0,
      end_time_seconds: segment.end_time_seconds || 0,
      transcription: segment.transcription || '',
      audio_base64: segment.audio_base64 || '',
      audio_data: segment.audio_data || [],
      isTranscribing: false,
      transcriptionError: null
    }));
    
    // Clear the file input
    target.value = '';
    
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
  projectName.value = "Untitled Project";
  errorMsg.value = "";
  transcriptionStatus.value = "";
  closeAllMenus();
}

function openAudioFile() {
  document.getElementById('audio-file')?.click();
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
              <span class="menu-icon">🎵</span>
              <span>Open Audio File...</span>
            </div>
            <div class="menu-separator"></div>
            <div class="menu-option" @click="saveProjectFromMenu" :class="{ disabled: vadResults.length === 0 }">
              <span class="menu-icon">💾</span>
              <span>Save Project</span>
              <span class="menu-shortcut">Ctrl+S</span>
            </div>
            <div class="menu-separator"></div>
            <div class="menu-submenu" v-if="vadResults.length > 0">
              <span class="menu-icon">📤</span>
              <span>Export</span>
              <span class="menu-arrow">▶</span>
              <div class="submenu">
                <div class="menu-option" @click="exportAsText">
                  <span class="menu-icon">📄</span>
                  <span>Text (.txt)</span>
                  <span class="menu-shortcut">Ctrl+E</span>
                </div>
                <div class="menu-option" @click="exportAsMarkdown">
                  <span class="menu-icon">📝</span>
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
              <span class="menu-icon">🔲</span>
              <span>Select All</span>
              <span class="menu-shortcut">Ctrl+A</span>
            </div>
            <div class="menu-option" @click="copyAll" :class="{ disabled: vadResults.length === 0 }">
              <span class="menu-icon">📋</span>
              <span>Copy All Text</span>
              <span class="menu-shortcut">Ctrl+C</span>
            </div>
          </div>
        </div>
        
        <div class="menu-item" @click.stop="toggleHelpMenu">
          Help
          <div v-if="showHelpMenu" class="dropdown-menu">
            <div class="menu-option disabled">
              <span class="menu-icon">ℹ️</span>
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
          @keydown.enter="$event.target.blur()"
        />
      </div>
      
      <div class="toolbar-right">
        <div class="status-indicator" v-if="isProcessing">
          <span class="spinner">⏳</span>
          Processing...
        </div>
        <div class="status-indicator" v-else-if="isTranscribing">
          <span class="spinner">⏳</span>
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

    <!-- Main Content Area -->
    <div class="main-content">
      <!-- Processing Indicator -->
      <div v-if="isProcessing" class="processing-section">
        <div class="processing-card">
          <div class="processing-spinner">⏳</div>
          <h3>Processing Audio</h3>
          <p v-if="transcriptionStatus">{{ transcriptionStatus }}</p>
          <p v-else>Please wait while we process your audio file...</p>
          <div class="processing-steps">
            <small>• Reading and analyzing audio file</small><br>
            <small>• Detecting speech segments with AI</small><br>
            <small>• Preparing for transcription</small>
          </div>
        </div>
      </div>

      <!-- Audio Upload Section -->
      <div v-else-if="!vadResults.length" class="upload-section">
        <div class="upload-card">
          <h2>Get Started</h2>
          <p>Upload an audio file to begin transcription</p>
          <button @click="openAudioFile" class="primary-button">
            🎵 Select Audio File
          </button>
          <p class="file-hint">Supports WAV, MP3, M4A, AAC, FLAC, and OGG formats</p>
        </div>
      </div>

      <div v-if="errorMsg" class="error">
        {{ errorMsg }}
      </div>

      <div v-if="vadResults.length > 0" class="results">
        <h3>Transcription Editor</h3>
        <p v-if="transcriptionStatus" class="transcription-status">{{ transcriptionStatus }}</p>
        
        <div class="transcription-editor">
          <div class="editor-controls">
            <span class="segment-count">{{ vadResults.filter(s => s.transcription).length }} segments</span>
            <span class="total-duration">Total: {{ formatTime(vadResults[vadResults.length - 1]?.end_time_seconds || 0) }}</span>
          </div>
          
          <div class="transcription-text">
            <div v-for="(segment, index) in vadResults" :key="index" class="segment-line">
              <div v-if="segment.transcription" class="segment-container">
                <div class="segment-meta">
                  <span class="segment-number">{{ index + 1 }}</span>
                  <span class="segment-time">{{ formatTime(segment.start_time_seconds) }}</span>
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
              <div v-else-if="segment.isTranscribing" class="segment-container loading">
                <div class="segment-meta">
                  <span class="segment-number">{{ index + 1 }}</span>
                  <span class="segment-time">{{ formatTime(segment.start_time_seconds) }}</span>
                </div>
                <span class="transcribing-segment">⏳ Transcribing...</span>
              </div>
              <div v-else-if="segment.transcriptionError" class="segment-container error">
                <div class="segment-meta">
                  <span class="segment-number">{{ index + 1 }}</span>
                  <span class="segment-time">{{ formatTime(segment.start_time_seconds) }}</span>
                </div>
                <span class="error-segment" :title="`Error: ${segment.transcriptionError}`">❌ Error</span>
              </div>
              <div v-else class="segment-container processing">
                <div class="segment-meta">
                  <span class="segment-number">{{ index + 1 }}</span>
                  <span class="segment-time">{{ formatTime(segment.start_time_seconds) }}</span>
                </div>
                <span class="processing-segment">⏳ Processing...</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </main>
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
  font-size: 14px;
  width: 16px;
  text-align: center;
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
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

/* Main Content */
.main-content {
  flex: 1;
  padding: 20px;
}

.processing-section {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 60vh;
}

.processing-card {
  background: white;
  padding: 40px;
  border-radius: 12px;
  box-shadow: 0 4px 20px rgba(0,0,0,0.1);
  text-align: center;
  max-width: 400px;
  border-left: 4px solid #ff9800;
}

.processing-spinner {
  font-size: 48px;
  animation: spin 1s linear infinite;
  margin-bottom: 16px;
}

.processing-card h3 {
  margin: 0 0 16px 0;
  color: #333;
  font-size: 24px;
  font-weight: 600;
}

.processing-card p {
  margin: 0 0 20px 0;
  color: #666;
  font-size: 16px;
}

.processing-steps {
  background: #f8f9fa;
  padding: 16px;
  border-radius: 8px;
  text-align: left;
  color: #666;
  font-size: 14px;
  line-height: 1.6;
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
}

</style>