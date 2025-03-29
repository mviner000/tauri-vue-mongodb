<!-- src/components/MongoDBDataTable.vue -->
<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { ReloadIcon, TrashIcon } from '@radix-icons/vue';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@/components/ui/table';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Pagination,
  PaginationList,
  PaginationListItem,
  PaginationFirst,
  PaginationLast,
  PaginationNext,
  PaginationPrev,
} from '@/components/ui/pagination';

const collectionName = ref('users');
const documents = ref<any[]>([]);
const isLoading = ref(false);
const errorMessage = ref('');
const pageSize = ref(10);
const currentPage = ref(1);
const filterQuery = ref('{}');

// Inline editing states
const editingCell = ref<{rowIndex: number; header: string} | null>(null);
const editValue = ref('');
const isSaving = ref(false);

// For pagination
const totalPages = computed(() => Math.ceil(documents.value.length / pageSize.value));
const paginatedDocuments = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value;
  const end = start + pageSize.value;
  return documents.value.slice(start, end);
});

// Get all unique keys from all documents for table headers
const tableHeaders = computed(() => {
  const headers = new Set<string>();
  headers.add('_id');
  documents.value.forEach(doc => {
    Object.keys(doc).forEach(key => headers.add(key));
  });
  return Array.from(headers);
});

function formatValue(value: any): string {
  if (value === undefined || value === null) return '';
  return typeof value === 'object' ? JSON.stringify(value) : String(value);
}

const fetchDocuments = async () => {
  isLoading.value = true;
  errorMessage.value = '';
  try {
    let filter = {};
    try {
      filter = JSON.parse(filterQuery.value);
    } catch (error) {
      errorMessage.value = `Invalid filter JSON: ${error}`;
      return;
    }
    documents.value = await invoke<any[]>('find_documents', {
      collectionName: collectionName.value,
      filter: filter
    });
    currentPage.value = 1;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (message.includes('Database connection not initialized')) {
      setTimeout(fetchDocuments, 1000);
    } else {
      errorMessage.value = `Error fetching documents: ${message}`;
      documents.value = [];
    }
  } finally {
    isLoading.value = false;
  }
};

const handleCellClick = (rowIndex: number, header: string, value: any) => {
  if (header === '_id') return;
  editingCell.value = { rowIndex, header };
  editValue.value = typeof value === 'object' ? JSON.stringify(value, null, 2) : String(value);
};

const saveEdit = async () => {
  if (!editingCell.value || isSaving.value) return;
  isSaving.value = true;
  
  const { rowIndex, header } = editingCell.value;
  const doc = paginatedDocuments.value[rowIndex];
  const originalDoc = documents.value.find(d => d._id.$oid === doc._id.$oid);
  if (!originalDoc) return;

  try {
    // Parse value based on original type
    let newValue: any = editValue.value;
    if (typeof originalDoc[header] === 'number') {
      newValue = parseFloat(editValue.value);
    } else if (typeof originalDoc[header] === 'object') {
      newValue = JSON.parse(editValue.value);
    }

    const update = { [header]: newValue };
    const success = await invoke<boolean>('update_document', {
      collectionName: collectionName.value,
      id: doc._id.$oid,
      update: update
    });

    if (success) {
      originalDoc[header] = newValue;
      documents.value = [...documents.value];
    }
    editingCell.value = null;
  } catch (error) {
    errorMessage.value = `Error updating field: ${error}`;
  } finally {
    isSaving.value = false;
  }
};

const deleteDocument = async (id: string) => {
  if (!confirm('Are you sure you want to delete this document?')) return;
  try {
    const success = await invoke<boolean>('delete_document', {
      collectionName: collectionName.value,
      id: id
    });
    if (success) fetchDocuments();
  } catch (error) {
    errorMessage.value = `Error deleting document: ${error}`;
  }
};

const collectionsList = ref<string[]>([]);
const fetchCollections = async () => {
  try {
    collectionsList.value = await invoke<string[]>('list_collections');
    if (collectionsList.value.length > 0 && !collectionsList.value.includes(collectionName.value)) {
      collectionName.value = collectionsList.value[0];
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (message.includes('Database connection not initialized')) {
      setTimeout(fetchCollections, 1000);
    } else {
      errorMessage.value = `Error fetching collections: ${message}`;
    }
  }
};

onMounted(() => {
  fetchCollections();
});

defineExpose({ fetchDocuments, fetchCollections });

const onPageChange = (page: number) => {
  currentPage.value = page;
};

watch(collectionName, fetchDocuments);
</script>

<template>
  <div class="border rounded-md p-4 w-full">
    <h2 class="text-xl font-bold mb-4">MongoDB Data Table</h2>
    
    <div class="flex flex-col md:flex-row gap-4 mb-4">
      <div class="w-full md:w-1/4">
        <Label for="collection-select">Collection</Label>
        <Select v-model="collectionName">
          <SelectTrigger class="mt-1">
            <SelectValue placeholder="Select a collection" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="collection in collectionsList" :key="collection" :value="collection">
              {{ collection }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
      
      <div class="w-full md:w-3/4">
        <Label for="filter-input">Filter Query (JSON)</Label>
        <div class="flex gap-2 mt-1">
          <Input id="filter-input" v-model="filterQuery" placeholder="{}" />
          <Button @click="fetchDocuments" :disabled="isLoading">
            <ReloadIcon v-if="isLoading" class="mr-2 h-4 w-4 animate-spin" />
            <span v-else>Filter</span>
          </Button>
        </div>
      </div>
    </div>
    
    <div v-if="errorMessage" class="my-2 p-2 bg-red-100 text-red-700 rounded">
      {{ errorMessage }}
    </div>
    
    <div v-if="isLoading" class="flex justify-center my-8">
      <ReloadIcon class="h-8 w-8 animate-spin text-gray-500" />
    </div>
    
    <div v-else-if="documents.length === 0" class="text-center my-8 text-gray-500">
      No documents found in collection "{{ collectionName }}"
    </div>
    
    <div v-else class="w-full overflow-auto">
      <Table class="excel-style-table">
        <TableHeader>
          <TableRow>
            <TableHead v-for="header in tableHeaders" :key="header" 
              class="bg-gray-100 font-semibold border-r border-b border-gray-300">
              {{ header }}
            </TableHead>
            <TableHead class="bg-gray-100 font-semibold border-b border-gray-300 text-right">
              Actions
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="(doc, rowIndex) in paginatedDocuments" :key="rowIndex">
            <TableCell v-for="header in tableHeaders" :key="`${rowIndex}-${header}`" 
              class="border-r border-b border-gray-200 p-0">
              <div class="h-full">
                <div v-if="editingCell?.rowIndex === rowIndex && editingCell?.header === header" 
                  class="h-full">
                  <Input v-if="typeof doc[header] !== 'object'"
                    v-model="editValue"
                    @blur="saveEdit"
                    @keyup.enter="saveEdit"
                    class="h-full rounded-none border-none focus-visible:ring-2"
                  />
                  <textarea v-else
                    v-model="editValue"
                    @blur="saveEdit"
                    @keyup.ctrl.enter="saveEdit"
                    class="w-full h-full p-2 font-mono text-sm border-none focus:ring-2 focus:ring-blue-500"
                    rows="3"
                  />
                </div>
                <div v-else
                  class="p-2 cursor-pointer hover:bg-blue-50 min-h-[40px]"
                  @click="handleCellClick(rowIndex, header, doc[header])"
                >
                  {{ formatValue(doc[header]) }}
                </div>
              </div>
            </TableCell>
            <TableCell class="border-b border-gray-200 text-right p-1">
              <Button
                variant="ghost"
                size="sm"
                class="text-red-600 hover:text-red-800"
                @click="deleteDocument(doc._id.$oid)"
              >
                <TrashIcon class="h-4 w-4" />
              </Button>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
      
      <div v-if="totalPages > 1" class="mt-4">
        <Pagination :page="currentPage" :itemsPerPage="pageSize" :total="documents.length"
          @update:page="onPageChange" :siblingCount="1">
          <PaginationList>
            <PaginationListItem :value="1">
              <PaginationFirst :disabled="currentPage === 1" @click="currentPage = 1" />
            </PaginationListItem>
            <PaginationListItem :value="Math.max(1, currentPage - 1)">
              <PaginationPrev :disabled="currentPage === 1" 
                @click="currentPage = Math.max(1, currentPage - 1)" />
            </PaginationListItem>
            <PaginationListItem :value="Math.min(totalPages, currentPage + 1)">
              <PaginationNext :disabled="currentPage === totalPages" 
                @click="currentPage = Math.min(totalPages, currentPage + 1)" />
            </PaginationListItem>
            <PaginationListItem :value="totalPages">
              <PaginationLast :disabled="currentPage === totalPages" 
                @click="currentPage = totalPages" />
            </PaginationListItem>
          </PaginationList>
        </Pagination>
      </div>
      
      <div class="flex items-center gap-2 mt-4">
        <span class="text-sm text-gray-500">Rows per page:</span>
        <Select v-model="pageSize">
          <SelectTrigger class="w-16">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="5">5</SelectItem>
            <SelectItem value="10">10</SelectItem>
            <SelectItem value="20">20</SelectItem>
            <SelectItem value="50">50</SelectItem>
            <SelectItem value="100">100</SelectItem>
          </SelectContent>
        </Select>
        
        <span class="ml-4 text-sm text-gray-500">
          Showing {{ (currentPage - 1) * pageSize + 1 }} to 
          {{ Math.min(currentPage * pageSize, documents.length) }} of 
          {{ documents.length }} entries
        </span>
      </div>
    </div>
  </div>
</template>

<style>
.excel-style-table {
  border-collapse: collapse;
  border-spacing: 0;
}

.excel-style-table th,
.excel-style-table td {
  @apply border-gray-300;
  border-style: solid;
  border-width: 0 1px 1px 0;
}

.excel-style-table th {
  @apply bg-gray-100;
}

.excel-style-table td {
  @apply bg-white;
}

.excel-style-table tr:hover td {
  @apply bg-gray-50;
}

.excel-style-table input {
  @apply h-full min-h-[40px] rounded-none border-none shadow-none focus-visible:ring-2;
}

.excel-style-table textarea {
  @apply min-h-[80px] resize-y;
}
</style>