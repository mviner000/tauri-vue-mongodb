<!-- src/components/MongoDBOperations.vue -->
<script setup lang="ts">
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Label } from '@/components/ui/label';
import { Card, CardContent, CardHeader, CardTitle, CardDescription, CardFooter } from '@/components/ui/card';
import { ReloadIcon } from '@radix-icons/vue';

// Create a way to communicate with other components
const emits = defineEmits(['documentAction']);

const collectionName = ref('users');
const documentId = ref('');
const documentData = ref('{\n  "name": "John Doe",\n  "email": "john@example.com",\n  "age": 30\n}');
const updateData = ref('{\n  "name": "Updated Name"\n}');
const resultMessage = ref('');
const isLoading = ref(false);
const isError = ref(false);

// Helper to parse JSON safely
function parseJsonSafely(jsonStr: string, defaultValue: any = {}) {
  try {
    return JSON.parse(jsonStr);
  } catch (error) {
    isError.value = true;
    resultMessage.value = `Invalid JSON: ${error}`;
    return defaultValue;
  }
}

async function insertDocument() {
  isLoading.value = true;
  isError.value = false;
  resultMessage.value = '';
  
  try {
    const docData = parseJsonSafely(documentData.value);
    if (isError.value) return;
    
    const id = await invoke<string>('insert_document', {
      collectionName: collectionName.value,
      document: docData
    });
    
    resultMessage.value = `Document inserted with ID: ${id}`;
    documentId.value = id; // Store the ID for further operations
    
    // Emit event to notify other components
    emits('documentAction', {
      type: 'insert',
      collectionName: collectionName.value
    });
  } catch (error) {
    isError.value = true;
    resultMessage.value = `Error inserting document: ${error}`;
  } finally {
    isLoading.value = false;
  }
}

async function updateDocument() {
  if (!documentId.value) {
    isError.value = true;
    resultMessage.value = 'Document ID is required for update';
    return;
  }
  
  isLoading.value = true;
  isError.value = false;
  resultMessage.value = '';
  
  try {
    const updateDoc = parseJsonSafely(updateData.value);
    if (isError.value) return;
    
    const success = await invoke<boolean>('update_document', {
      collectionName: collectionName.value,
      id: documentId.value,
      update: updateDoc
    });
    
    if (success) {
      resultMessage.value = 'Document updated successfully';
      
      // Emit event to notify other components
      emits('documentAction', {
        type: 'update',
        collectionName: collectionName.value
      });
    } else {
      resultMessage.value = 'No document was updated (ID might not exist)';
    }
  } catch (error) {
    isError.value = true;
    resultMessage.value = `Error updating document: ${error}`;
  } finally {
    isLoading.value = false;
  }
}

async function deleteDocument() {
  if (!documentId.value) {
    isError.value = true;
    resultMessage.value = 'Document ID is required for deletion';
    return;
  }
  
  isLoading.value = true;
  isError.value = false;
  resultMessage.value = '';
  
  try {
    const success = await invoke<boolean>('delete_document', {
      collectionName: collectionName.value,
      id: documentId.value
    });
    
    if (success) {
      resultMessage.value = 'Document deleted successfully';
      documentId.value = ''; // Clear ID after deletion
      
      // Emit event to notify other components
      emits('documentAction', {
        type: 'delete',
        collectionName: collectionName.value
      });
    } else {
      resultMessage.value = 'No document was deleted (ID might not exist)';
    }
  } catch (error) {
    isError.value = true;
    resultMessage.value = `Error deleting document: ${error}`;
  } finally {
    isLoading.value = false;
  }
}
</script>

<template>
  <div class="border rounded-md p-4 w-full max-w-xl">
    <h2 class="text-xl font-bold mb-4">MongoDB Operations</h2>
    
    <div class="space-y-4">
      <!-- Collection name input -->
      <div>
        <Label for="collection-name">Collection Name</Label>
        <Input 
          id="collection-name" 
          v-model="collectionName" 
          placeholder="users"
          class="mt-1"
        />
      </div>
      
      <!-- Insert Document Section -->
      <Card>
        <CardHeader>
          <CardTitle>Insert Document</CardTitle>
          <CardDescription>Add a new document to the collection</CardDescription>
        </CardHeader>
        <CardContent>
          <Label for="document-data">Document Data (JSON)</Label>
          <Textarea 
            id="document-data" 
            v-model="documentData" 
            rows="5" 
            class="font-mono mt-1"
          />
        </CardContent>
        <CardFooter>
          <Button @click="insertDocument" :disabled="isLoading">
            <ReloadIcon v-if="isLoading" class="mr-2 h-4 w-4 animate-spin" />
            Insert Document
          </Button>
        </CardFooter>
      </Card>
      
      <!-- Document ID for Update/Delete -->
      <div>
        <Label for="document-id">Document ID (for Update/Delete)</Label>
        <Input 
          id="document-id" 
          v-model="documentId" 
          placeholder="ObjectId"
          class="mt-1"
        />
      </div>
      
      <!-- Update Document Section -->
      <Card>
        <CardHeader>
          <CardTitle>Update Document</CardTitle>
          <CardDescription>Modify an existing document</CardDescription>
        </CardHeader>
        <CardContent>
          <Label for="update-data">Update Data (JSON)</Label>
          <Textarea 
            id="update-data" 
            v-model="updateData" 
            rows="3" 
            class="font-mono mt-1"
          />
        </CardContent>
        <CardFooter>
          <Button @click="updateDocument" :disabled="isLoading || !documentId">
            <ReloadIcon v-if="isLoading" class="mr-2 h-4 w-4 animate-spin" />
            Update Document
          </Button>
        </CardFooter>
      </Card>
      
      <!-- Delete Document Button -->
      <Button 
        @click="deleteDocument" 
        variant="destructive" 
        :disabled="isLoading || !documentId"
        class="w-full"
      >
        <ReloadIcon v-if="isLoading" class="mr-2 h-4 w-4 animate-spin" />
        Delete Document
      </Button>
      
      <!-- Results Display -->
      <div v-if="resultMessage" class="mt-4">
        <p :class="{ 'text-red-500': isError, 'text-green-500': !isError }">
          {{ resultMessage }}
        </p>
      </div>
    </div>
  </div>
</template>