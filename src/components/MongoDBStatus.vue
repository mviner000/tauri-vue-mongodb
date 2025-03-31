<!-- src/components/MongoDBStatus.vue -->
<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '@/components/ui/button';
import { ReloadIcon } from '@radix-icons/vue';
import { listen } from '@tauri-apps/api/event';

const mongoDBStatus = ref<'checking' | 'installed' | 'not-installed'>('checking');
const isLoading = ref(true);
const isInstalling = ref(false);
const installError = ref<string | null>(null);
const installLogs = ref<string[]>([]);
const isConnected = ref(false);
const connectionString = ref('mongodb://localhost:27017');
const installerPath = ref<string | null>(null);

// Download progress tracking
const downloadProgress = ref({
  bytesDownloaded: 0,
  totalBytes: 0,
  percentage: 0,
  isDownloading: false
});

// Format bytes to human-readable format
const formatBytes = (bytes: number): string => {
  if (bytes === 0) return '0 Bytes';
  
  const k = 1024;
  const sizes = ['Bytes', 'KB', 'MB', 'GB'];
  
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
};

// Computed properties for readable values
const readableDownloaded = computed(() => formatBytes(downloadProgress.value.bytesDownloaded));
const readableTotal = computed(() => formatBytes(downloadProgress.value.totalBytes));
const progressPercentage = computed(() => 
  Math.round(downloadProgress.value.percentage)
);

async function checkMongoDB() {
  try {
    const isInstalled = await invoke<boolean>('is_mongodb_installed');
    mongoDBStatus.value = isInstalled ? 'installed' : 'not-installed';
  } catch (error) {
    console.error('Error checking MongoDB:', error);
    mongoDBStatus.value = 'not-installed';
  } finally {
    isLoading.value = false;
  }
}

async function handleInstall() {
  isInstalling.value = true;
  installError.value = null;
  installLogs.value = [];
  downloadProgress.value.isDownloading = false;
  downloadProgress.value.percentage = 0;
  downloadProgress.value.bytesDownloaded = 0;
  downloadProgress.value.totalBytes = 0;
  installerPath.value = null;
  
  try {
    await invoke<void>('install_mongodb');
    // Wait a bit longer for services to start
    await new Promise(resolve => setTimeout(resolve, 5000));
    const isInstalled = await invoke<boolean>('is_mongodb_installed');
    mongoDBStatus.value = isInstalled ? 'installed' : 'not-installed';
    
    if (!isInstalled) {
      installError.value = "MongoDB installation seemed to complete but verification failed";
    }
  } catch (error) {
    console.error('Installation failed:', error);
    installError.value = typeof error === 'string' ? error : 'Installation failed';
  } finally {
    isInstalling.value = false;
    downloadProgress.value.isDownloading = false;
  }
}

async function connectToMongoDB() {
  try {
    await invoke<void>('connect_mongodb', { connectionString: connectionString.value });
    isConnected.value = true;
  } catch (error) {
    console.error('Connection failed:', error);
    isConnected.value = false;
    installError.value = typeof error === 'string' ? error : 'Connection failed';
  }
}

async function disconnectFromMongoDB() {
  try {
    await invoke<void>('disconnect_mongodb');
    isConnected.value = false;
  } catch (error) {
    console.error('Disconnect failed:', error);
  }
}

// Listen for installation logs
listen('mongodb-install-log', (event) => {
    const payload = event.payload as any;
    // Check if this is a structured payload or a string
    if (typeof payload === 'object' && payload.message) {
        installLogs.value.unshift(payload.message);
        
        // Set download state based on step messages
        if (payload.message.includes("Downloading MongoDB installer")) {
            downloadProgress.value.isDownloading = true;
        } else if (payload.step > 2) { // Any step after downloading
            downloadProgress.value.isDownloading = false;
        }
    } else {
        // Handle string payload for backward compatibility
        installLogs.value.unshift(payload as string);
    }
});

listen('mongodb-install-error', (event) => {
    const payload = event.payload as any;
    if (typeof payload === 'object' && payload.message) {
        installLogs.value.unshift(`ERROR: ${payload.message}`);
    } else {
        installLogs.value.unshift(`ERROR: ${payload}`);
    }
});

// Listen for installer path information
listen('mongodb-installer-path', (event) => {
    const path = event.payload as string;
    if (path) {
        installerPath.value = path;
        installLogs.value.unshift(`Installer location: ${path}`);
    }
});

// Listen for download progress updates
listen('mongodb-download-progress', (event) => {
    const progress = event.payload as any;
    if (progress && typeof progress === 'object') {
        downloadProgress.value.bytesDownloaded = progress.bytes_downloaded || 0;
        downloadProgress.value.totalBytes = progress.total_bytes || 0;
        downloadProgress.value.percentage = progress.percentage || 0;
        downloadProgress.value.isDownloading = true;
        
        // Add current progress to logs if it's a milestone (every 20% or when complete)
        const percentage = Math.round(progress.percentage);
        if (percentage % 20 === 0 || percentage === 100) {
            const progressMessage = `Downloaded ${formatBytes(progress.bytes_downloaded)} of ${formatBytes(progress.total_bytes)} (${percentage}%)`;
            // Only add if it's a new milestone
            if (!installLogs.value.some(log => log.includes(`(${percentage}%)`))) {
                installLogs.value.unshift(progressMessage);
            }
        }
    }
});

onMounted(() => {
  checkMongoDB();
});
</script>

<template>
  <div class="border rounded-md p-4 w-full max-w-xl">
    <h2 class="text-xl font-bold mb-4">MongoDB Status</h2>
    
    <!-- Download progress bar (visible only during download) -->
    <div v-if="downloadProgress.isDownloading" class="mb-4">
      <div class="flex justify-between text-sm mb-1">
        <span>Downloading MongoDB ({{ progressPercentage }}%)</span>
        <span>{{ readableDownloaded }} / {{ readableTotal }}</span>
      </div>
      <div class="w-full bg-gray-200 rounded-full h-4 dark:bg-gray-700">
        <div 
          class="bg-blue-600 h-4 rounded-full transition-all duration-300 ease-out" 
          :style="{ width: `${downloadProgress.percentage}%` }"
        ></div>
      </div>
    </div>
    
    <!-- Installer path info (when available) -->
    <div v-if="installerPath" class="mb-4 p-3 bg-blue-50 text-blue-700 rounded-md text-sm">
      <div class="font-medium mb-1">Installer Location:</div>
      <div class="font-mono break-all">{{ installerPath }}</div>
    </div>
    
    <!-- Installation logs - reversed to show latest first -->
    <div v-if="installLogs.length > 0" class="mb-4 border rounded-md p-2 max-h-48 overflow-y-auto bg-gray-100">
      <div v-for="(log, index) in installLogs" :key="index" class="text-sm font-mono">
        {{ log }}
      </div>
    </div>

    <!-- Status display -->
    <div class="flex items-center gap-2 mb-4">
      <Button variant="outline" @click="checkMongoDB" :disabled="isLoading || isInstalling">
        <template v-if="isLoading">
          <ReloadIcon class="mr-2 h-4 w-4 animate-spin" />
          Checking...
        </template>
        <span v-else>Refresh Status</span>
      </Button>
      
      <div class="flex items-center gap-2 ml-2">
        <template v-if="mongoDBStatus === 'installed'">
          <div class="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
          <span class="text-green-500">MongoDB is installed</span>
        </template>
        <template v-else-if="mongoDBStatus === 'checking'">
          <div class="w-2 h-2 rounded-full bg-yellow-500 animate-pulse" />
          <span class="text-yellow-500">Checking MongoDB status...</span>
        </template>
        <template v-else>
          <div class="w-2 h-2 rounded-full bg-red-500" />
          <span class="text-red-500">MongoDB not found</span>
        </template>
      </div>
    </div>

    <!-- Installation controls -->
    <div v-if="mongoDBStatus === 'not-installed'" class="mb-4">
      <Button 
        variant="default" 
        @click="handleInstall"
        :disabled="isInstalling"
        class="w-full"
      >
        <template v-if="isInstalling">
          <ReloadIcon class="mr-2 h-4 w-4 animate-spin" />
          <span v-if="downloadProgress.isDownloading">
            Downloading MongoDB ({{ progressPercentage }}%)
          </span>
          <span v-else>
            Installing MongoDB...
          </span>
        </template>
        <span v-else>Install MongoDB</span>
      </Button>
      
      <p v-if="installError" class="text-sm text-red-500 text-center mt-2">
        {{ installError }}<br>
        <span class="text-muted-foreground">Please check your permissions and internet connection</span>
      </p>
      
      <p class="text-sm text-muted-foreground text-center mt-2">
        This will install MongoDB 8.0.6. Requires administrator privileges.
      </p>
    </div>

    <!-- Connection controls (only show when installed) -->
    <div v-if="mongoDBStatus === 'installed'" class="space-y-4">
      <div>
        <label for="connection-string" class="block text-sm font-medium mb-1">Connection String</label>
        <input 
          id="connection-string" 
          v-model="connectionString" 
          placeholder="mongodb://localhost:27017"
          class="w-full p-2 border rounded-md"
        />
      </div>
      
      <div class="flex gap-2">
        <Button 
          v-if="!isConnected" 
          variant="default" 
          @click="connectToMongoDB"
          class="flex-1"
        >
          Connect
        </Button>
        <Button 
          v-else 
          variant="destructive" 
          @click="disconnectFromMongoDB"
          class="flex-1"
        >
          Disconnect
        </Button>
      </div>
      
      <div v-if="isConnected" class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
        <span class="text-green-500">Connected to MongoDB</span>
      </div>
      
      <p v-if="installError" class="text-sm text-red-500">
        {{ installError }}
      </p>
    </div>
  </div>
</template>