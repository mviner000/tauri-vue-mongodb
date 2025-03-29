// src/mongodb_manager.rs

use mongodb::{Client, Database, options::ClientOptions};
use mongodb::bson::Document;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::State;
use anyhow::Result;
use futures_util::stream::StreamExt; // Add this import for cursor.next()

// Define MongoDB connection state
pub struct MongoDbState {
    client: Arc<Mutex<Option<Client>>>,
    database_name: String,
}

impl MongoDbState {
    pub fn new(database_name: &str) -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            database_name: database_name.to_string(),
        }
    }

    pub async fn get_database(&self) -> Result<Database, String> {
        let client_guard = self.client.lock().await;
        
        if client_guard.is_none() {
            return Err("Database connection not initialized. Call connect() first.".into());
        }
        
        let client = client_guard.as_ref().unwrap();
        Ok(client.database(&self.database_name))
    }
}

#[tauri::command]
pub async fn connect_mongodb(
    mongodb_state: State<'_, MongoDbState>,
    connection_string: String,
) -> Result<(), String> {
    let mut client_guard = mongodb_state.client.lock().await;
    
    if client_guard.is_some() {
        // Already connected
        return Ok(());
    }
    
    // Parse connection string and create client options
    let client_options = ClientOptions::parse(&connection_string)
        .await
        .map_err(|e| format!("Failed to parse connection string: {}", e))?;
    
    // Create a new client
    let client = Client::with_options(client_options)
        .map_err(|e| format!("Failed to create MongoDB client: {}", e))?;
    
    // Test the connection by pinging the server
    client
        .database("admin")
        .run_command(mongodb::bson::doc! { "ping": 1 }, None)
        .await
        .map_err(|e| format!("Failed to connect to MongoDB: {}", e))?;
    
    // Store the client
    *client_guard = Some(client);
    
    Ok(())
}

#[tauri::command]
pub async fn disconnect_mongodb(mongodb_state: State<'_, MongoDbState>) -> Result<(), String> {
    let mut client_guard = mongodb_state.client.lock().await;
    *client_guard = None;
    Ok(())
}

// Insert document function (not generic)
#[tauri::command]
pub async fn insert_document(
    mongodb_state: State<'_, MongoDbState>,
    collection_name: String,
    document: Document, // Use concrete Document type
) -> Result<String, String> {
    let db = mongodb_state.get_database().await?;
    let collection = db.collection::<Document>(&collection_name);
    
    let result = collection.insert_one(document, None)
        .await
        .map_err(|e| format!("Failed to insert document: {}", e))?;
    
    match result.inserted_id.as_object_id() {
        Some(id) => Ok(id.to_hex()),
        None => Err("Failed to get inserted document ID".into()),
    }
}

// Find documents function (not generic)
#[tauri::command]
pub async fn find_documents(
    mongodb_state: State<'_, MongoDbState>,
    collection_name: String,
    filter: Document, // Use concrete Document type
) -> Result<Vec<Document>, String> {
    let db = mongodb_state.get_database().await?;
    let collection = db.collection::<Document>(&collection_name);
    
    let mut cursor = collection.find(filter, None)
        .await
        .map_err(|e| format!("Failed to find documents: {}", e))?;
    
    let mut documents = Vec::new();
    while let Some(document_result) = cursor.next().await {
        match document_result {
            Ok(doc) => documents.push(doc),
            Err(e) => return Err(format!("Error retrieving document: {}", e)),
        }
    }
    
    Ok(documents)
}

// Update document by ID
#[tauri::command]
pub async fn update_document(
    mongodb_state: State<'_, MongoDbState>,
    collection_name: String,
    id: String,
    update: Document, // Use concrete Document type
) -> Result<bool, String> {
    let db = mongodb_state.get_database().await?;
    let collection = db.collection::<Document>(&collection_name);
    
    let object_id = mongodb::bson::oid::ObjectId::parse_str(&id)
        .map_err(|e| format!("Invalid ObjectId: {}", e))?;
    
    let filter = mongodb::bson::doc! { "_id": object_id };
    let update_doc = mongodb::bson::doc! { "$set": update };
    
    let result = collection.update_one(filter, update_doc, None)
        .await
        .map_err(|e| format!("Failed to update document: {}", e))?;
    
    Ok(result.modified_count > 0)
}

// Delete document by ID
#[tauri::command]
pub async fn delete_document(
    mongodb_state: State<'_, MongoDbState>,
    collection_name: String,
    id: String,
) -> Result<bool, String> {
    let db = mongodb_state.get_database().await?;
    let collection = db.collection::<Document>(&collection_name);
    
    let object_id = mongodb::bson::oid::ObjectId::parse_str(&id)
        .map_err(|e| format!("Invalid ObjectId: {}", e))?;
    
    let filter = mongodb::bson::doc! { "_id": object_id };
    
    let result = collection.delete_one(filter, None)
        .await
        .map_err(|e| format!("Failed to delete document: {}", e))?;
    
    Ok(result.deleted_count > 0)
}

pub async fn auto_connect(mongodb_state: &MongoDbState) -> Result<(), String> {
    let connection_string = "mongodb://localhost:27017";
    let mut client_guard = mongodb_state.client.lock().await;
    
    if client_guard.is_some() {
        return Ok(());
    }
    
    let client_options = ClientOptions::parse(connection_string)
        .await
        .map_err(|e| format!("Failed to parse connection string: {}", e))?;
    
    let client = Client::with_options(client_options)
        .map_err(|e| format!("Failed to create MongoDB client: {}", e))?;
    
    client
        .database("admin")
        .run_command(bson::doc! { "ping": 1 }, None)
        .await
        .map_err(|e| format!("Failed to connect to MongoDB: {}", e))?;
    
    *client_guard = Some(client);
    Ok(())
}

#[tauri::command]
pub async fn list_collections(
    mongodb_state: State<'_, MongoDbState>
) -> Result<Vec<String>, String> {
    let db = mongodb_state.get_database().await?;
    let filter = Some(bson::doc! {}); // Include all collections
    let collections = db.list_collection_names(filter)
        .await
        .map_err(|e| format!("Failed to list collections: {}", e))?;
    Ok(collections)
}