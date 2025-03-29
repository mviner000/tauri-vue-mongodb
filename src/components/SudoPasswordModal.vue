<!-- src/components/SudoPasswordModal.vue -->
<script setup lang="ts">
import { ref } from 'vue';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { listen } from '@tauri-apps/api/event';

const showModal = ref(false);
const password = ref('');
const currentRequestId = ref('');
const errorMessage = ref('');
const databaseType = ref('Database'); // Generic name, will be set dynamically

listen('sudo-password-request', (event: any) => {
    currentRequestId.value = event.payload.request_id;
    // Set the database type if provided in the payload
    if (event.payload.database_type) {
      databaseType.value = event.payload.database_type;
    } else {
      databaseType.value = 'Database'; // Default generic value
    }
    showModal.value = true;
    console.log("Received sudo password request with UUID:", event.payload.request_id);
});

async function submitPassword() {
    if (!password.value || !currentRequestId.value) return;
    
    try {
        const { emit } = await import('@tauri-apps/api/event');
        // Emit directly to the response channel
        await emit(`sudo-password-response-${currentRequestId.value}`, password.value);
        showModal.value = false;
        password.value = '';
        errorMessage.value = '';
    } catch (error) {
        errorMessage.value = `Error: ${error}`;
    }
}
</script>

<template>
    <Dialog :open="showModal" @update:open="showModal = $event">
        <DialogContent>
            <DialogHeader>
                <DialogTitle>Sudo Password Required</DialogTitle>
                <DialogDescription>
                    Please enter your sudo password to continue {{ databaseType }} installation
                </DialogDescription>
            </DialogHeader>
            
            <div class="grid gap-4 py-4">
                <div class="grid grid-cols-4 items-center gap-4">
                    <Label for="password" class="text-right">
                        Password
                    </Label>
                    <Input
                        id="password"
                        v-model="password"
                        type="password"
                        class="col-span-3"
                        @keyup.enter="submitPassword"
                    />
                </div>
                <div v-if="errorMessage" class="text-red-500 text-sm">
                    {{ errorMessage }}
                </div>
            </div>

            <DialogFooter>
                <Button @click="submitPassword">
                    Submit
                </Button>
            </DialogFooter>
        </DialogContent>
    </Dialog>
</template>