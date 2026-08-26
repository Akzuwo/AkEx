use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum IndexStatus {
    NotIndexed,
    Indexing,
    Ready,
    OutOfDate,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub id: i64,
    pub volume_id: String,
    pub root_path: String,
    pub label: Option<String>,
    pub filesystem_type: Option<String>,
    pub total_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
    pub last_full_scan: Option<String>,
    pub index_status: IndexStatus,
    pub entry_count: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub volume_id: i64,
    pub name: String,
    pub full_path: String,
    pub extension: Option<String>,
    pub is_directory: bool,
    pub size: u64,
    pub recursive_size: u64,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub hidden: bool,
    pub read_only: bool,
    pub system: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub offset: u64,
    pub limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub scan_id: String,
    pub root_path: String,
    pub entries_found: u64,
    pub bytes_found: u64,
    pub current_path: String,
    pub percent: Option<f64>,
    pub phase: String,
    pub errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationResult {
    pub ok: bool,
    pub integrity_message: String,
    pub orphan_count: u64,
    pub size_mismatch_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionUsage {
    pub extension: String,
    pub bytes: u64,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageAnalysis {
    pub total_bytes: u64,
    pub file_count: u64,
    pub folder_count: u64,
    pub largest_folders: Vec<Entry>,
    pub largest_files: Vec<Entry>,
    pub extensions: Vec<ExtensionUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    pub kind: String,
    pub mime_type: Option<String>,
    pub data: Option<String>,
    pub text: Option<String>,
    pub message: Option<String>,
}
