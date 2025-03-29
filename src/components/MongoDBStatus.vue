<!-- src/components/MongoDBStatus.vue -->
<script setup lang="ts">
import { ref, onMounted } from 'vue';
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
    // Add new logs to the beginning of the array for reverse chronological order
    installLogs.value.unshift(event.payload as string);
});

listen('mongodb-install-error', (event) => {
    // Add errors to the beginning of the array too
    installLogs.value.unshift(`ERROR: ${event.payload}`);
});

onMounted(() => {
  checkMongoDB();
});
</script>

<template>
  <div class="border rounded-md p-4 w-full max-w-xl">
    <h2 class="text-xl font-bold mb-4">MongoDB Status</h2>
    
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
          Installing MongoDB...
        </template>
        <span v-else>Install MongoDB</span>
      </Button>
      
      <p v-if="installError" class="text-sm text-red-500 text-center mt-2">
        {{ installError }}<br>
        <span class="text-muted-foreground">Please check your permissions and internet connection</span>
      </p>
      
      <p class="text-sm text-muted-foreground text-center mt-2">
        This will install MongoDB 8.0. Requires administrator privileges.
      </p>
    </div>

    <!-- Connection controls (only show when installed) -->
    <div v-if="mongoDBStatus === 'installed'" class="space-y-4">
      <div>
        <Label for="connection-string">Connection String</Label>
        <Input 
          id="connection-string" 
          v-model="connectionString" 
          placeholder="mongodb://localhost:27017"
          class="mt-1"
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