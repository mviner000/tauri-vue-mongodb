<!-- src/App.vue -->
<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import SudoPasswordModal from '@/components/SudoPasswordModal.vue';
import MongoDBStatus from '@/components/MongoDBStatus.vue';
import MongoDBOperations from '@/components/MongoDBOperations.vue';
import MongoDBDataTable from '@/components/MongoDBDataTable.vue';

// Reference to data table component for refreshing
const dataTableRef = ref<InstanceType<typeof MongoDBDataTable> | null>(null);
const isConnecting = ref(false);
const connectionError = ref('');

// Auto-connect to MongoDB on startup
async function autoConnectMongoDB() {
  isConnecting.value = true;
  connectionError.value = '';
  
  try {
    const isInstalled = await invoke<boolean>('is_mongodb_installed');
    if (isInstalled) {
      await invoke('connect_mongodb', {
        connectionString: 'mongodb://localhost:27017'
      });
      console.log('Successfully connected to MongoDB');
      
      // Add small delay to ensure connection is ready
      await new Promise(resolve => setTimeout(resolve, 500));
      
      // Refresh data table after connection
      if (dataTableRef.value) {
        dataTableRef.value.fetchDocuments();
      }
    }
  } catch (error) {
    connectionError.value = `Failed to connect to MongoDB: ${error}`;
  } finally {
    isConnecting.value = false;
  }
}

// Handle document operations
function handleDocumentAction(event: { type: string, collectionName: string }) {
  // Refresh the data table when document operations occur
  if (dataTableRef.value) {
    dataTableRef.value.fetchDocuments();
    dataTableRef.value.fetchCollections();
  }
}

// Try to connect on component mount
onMounted(() => {
  autoConnectMongoDB();
});
</script>

<template>
  <div class="container mx-auto p-4">
    <!-- Sudo password modal -->
    <SudoPasswordModal />
    <h1 class="text-2xl font-bold mb-6">MongoDB Database Manager</h1>
    
    <!-- Connection error message -->
    <div v-if="connectionError" class="mb-4 p-3 bg-red-100 text-red-700 rounded-md">
      {{ connectionError }}
    </div>
    
    <div class="grid gap-6 md:grid-cols-1 lg:grid-cols-2">
      <MongoDBStatus @connection-status-changed="autoConnectMongoDB" />
      <MongoDBOperations @document-action="handleDocumentAction" />
    </div>
    
    <!-- MongoDB Data Table -->
    <div class="mt-8">
      <MongoDBDataTable ref="dataTableRef" />
    </div>
  </div>
</template>