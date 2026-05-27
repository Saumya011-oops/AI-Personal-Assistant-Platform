use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use gray_matter::{engine::YAML, Matter};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::NormalizedDocument;

#[derive(Clone)]
pub struct ObsidianIntegration {
    vault_path: PathBuf,
}

impl ObsidianIntegration {
    pub fn new(vault_path: String) -> Self {
        Self {
            vault_path: PathBuf::from(vault_path),
        }
    }

    pub fn scan_documents(&self) -> Result<Vec<NormalizedDocument>> {
        let mut docs = Vec::new();
        self.walk_dir(&self.vault_path, &mut docs)?;
        Ok(docs)
    }

    fn walk_dir(&self, path: &Path, docs: &mut Vec<NormalizedDocument>) -> Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            if entry.file_type()?.is_dir() {
                self.walk_dir(&entry_path, docs)?;
                continue;
            }

            if entry_path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }

            let raw = fs::read_to_string(&entry_path)?;
            let matter = Matter::<YAML>::new();
            let parsed = matter.parse(&raw);
            let title = entry_path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("Untitled")
                .to_string();
            let checksum = format!("{:x}", Sha256::digest(raw.as_bytes()));
            let metadata = parsed
                .data
                .as_ref()
                .map(|frontmatter| serde_json::json!({ "frontmatter": format!("{frontmatter:?}") }))
                .unwrap_or_else(|| serde_json::json!({}));

            docs.push(NormalizedDocument {
                id: Uuid::new_v4().to_string(),
                source_kind: "obsidian".to_string(),
                source_external_id: entry_path.to_string_lossy().to_string(),
                title,
                content_markdown: raw.clone(),
                content_plaintext: parsed.content,
                path_or_url: Some(entry_path.to_string_lossy().to_string()),
                tags: Vec::new(),
                created_at: None,
                updated_at: None,
                checksum,
                metadata,
            });
        }
        Ok(())
    }
}
