use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::Arc,
};

use mime::Mime;
use tokio::{
    fs,
    io::{AsyncRead, AsyncWriteExt},
    sync::RwLock,
};
use uuid::Uuid;

pub const MAX_ATTACHMENT_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_ATTACHMENTS_PER_SUBMISSION: usize = 10;

#[derive(Debug, Clone)]
pub struct UploadInput {
    pub original_name: String,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

impl UploadInput {
    pub fn new(
        original_name: impl Into<String>,
        mime_type: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            original_name: original_name.into(),
            mime_type: mime_type.into(),
            bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAttachment {
    pub original_name: String,
    pub stored_name: String,
    pub mime_type: String,
    pub file_size: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("附件路径不合法")]
    InvalidPath,
    #[error("附件为空或超过大小限制")]
    InvalidSize,
    #[error("不支持的附件类型")]
    UnsupportedType,
    #[error("单项成果附件数量超过限制")]
    TooManyAttachments,
    #[error("附件文件操作失败")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct AttachmentStorage {
    root: Arc<PathBuf>,
    paths: Arc<RwLock<HashMap<String, PathBuf>>>,
}

impl AttachmentStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: Arc::new(root.into()),
            paths: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn root(&self) -> &Path {
        self.root.as_path()
    }

    pub async fn save(
        &self,
        year_name: &str,
        submission_no: &str,
        upload: UploadInput,
    ) -> Result<StoredAttachment, StorageError> {
        validate_component(year_name)?;
        validate_component(submission_no)?;
        let mime_type = normalize_mime(&upload.mime_type)?;
        validate_original_name(&upload.original_name, &mime_type)?;
        validate_size(upload.bytes.len() as u64)?;

        let stored_name = format!("{}{}", Uuid::new_v4().simple(), extension_for(&mime_type));
        let directory = self.root.join(year_name).join(submission_no);
        fs::create_dir_all(&directory).await?;
        let temporary = directory.join(format!(".upload-{}", Uuid::new_v4().simple()));
        let final_path = directory.join(&stored_name);
        let result = async {
            let mut file = fs::File::create(&temporary).await?;
            file.write_all(&upload.bytes).await?;
            file.flush().await?;
            file.sync_all().await?;
            fs::rename(&temporary, &final_path).await?;
            Ok::<_, std::io::Error>(())
        }
        .await;
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary).await;
            return Err(StorageError::Io(error));
        }
        self.paths
            .write()
            .await
            .insert(stored_name.clone(), final_path);
        Ok(StoredAttachment {
            original_name: upload.original_name,
            stored_name,
            mime_type,
            file_size: upload.bytes.len() as i64,
        })
    }

    pub async fn save_many(
        &self,
        year_name: &str,
        submission_no: &str,
        existing_count: usize,
        uploads: Vec<UploadInput>,
    ) -> Result<Vec<StoredAttachment>, StorageError> {
        validate_attachment_count(existing_count, uploads.len())?;
        let mut saved = Vec::with_capacity(uploads.len());
        for upload in uploads {
            match self.save(year_name, submission_no, upload).await {
                Ok(item) => saved.push(item),
                Err(error) => {
                    for item in &saved {
                        let _ = self
                            .remove_for_submission(year_name, submission_no, &item.stored_name)
                            .await;
                    }
                    return Err(error);
                }
            }
        }
        Ok(saved)
    }

    pub async fn open(&self, stored_name: &str) -> Result<fs::File, StorageError> {
        validate_stored_name(stored_name)?;
        if let Some(path) = self.paths.read().await.get(stored_name).cloned() {
            return Ok(fs::File::open(path).await?);
        }
        let mut pending = VecDeque::from([self.root.as_ref().clone()]);
        while let Some(directory) = pending.pop_front() {
            let mut entries = fs::read_dir(&directory).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if entry.file_type().await?.is_dir() {
                    pending.push_back(path);
                } else if path.file_name().and_then(|name| name.to_str()) == Some(stored_name) {
                    self.paths
                        .write()
                        .await
                        .insert(stored_name.to_owned(), path.clone());
                    return Ok(fs::File::open(path).await?);
                }
            }
        }
        Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "attachment not found",
        )))
    }

    pub async fn open_for_submission(
        &self,
        year_name: &str,
        submission_no: &str,
        stored_name: &str,
    ) -> Result<fs::File, StorageError> {
        validate_component(year_name)?;
        validate_component(submission_no)?;
        validate_stored_name(stored_name)?;
        Ok(fs::File::open(
            self.root
                .join(year_name)
                .join(submission_no)
                .join(stored_name),
        )
        .await?)
    }

    pub async fn remove_for_submission(
        &self,
        year_name: &str,
        submission_no: &str,
        stored_name: &str,
    ) -> Result<(), StorageError> {
        validate_component(year_name)?;
        validate_component(submission_no)?;
        validate_stored_name(stored_name)?;
        fs::remove_file(
            self.root
                .join(year_name)
                .join(submission_no)
                .join(stored_name),
        )
        .await?;
        self.paths.write().await.remove(stored_name);
        Ok(())
    }

    pub async fn open_reader(
        &self,
        stored_name: &str,
    ) -> Result<impl AsyncRead + Unpin, StorageError> {
        self.open(stored_name).await
    }
}

pub fn validate_attachment_count(
    existing_count: usize,
    incoming_count: usize,
) -> Result<(), StorageError> {
    if existing_count.saturating_add(incoming_count) > MAX_ATTACHMENTS_PER_SUBMISSION {
        Err(StorageError::TooManyAttachments)
    } else {
        Ok(())
    }
}

fn validate_component(value: &str) -> Result<(), StorageError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.chars().any(char::is_control)
    {
        return Err(StorageError::InvalidPath);
    }
    Ok(())
}

fn validate_stored_name(value: &str) -> Result<(), StorageError> {
    validate_component(value)?;
    let (stem, extension) = value.rsplit_once('.').ok_or(StorageError::InvalidPath)?;
    if stem.len() != 32
        || !stem.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !matches!(extension, "jpg" | "png" | "pdf")
    {
        return Err(StorageError::InvalidPath);
    }
    Ok(())
}

fn validate_size(size: u64) -> Result<(), StorageError> {
    if size == 0 || size > MAX_ATTACHMENT_BYTES {
        Err(StorageError::InvalidSize)
    } else {
        Ok(())
    }
}

fn normalize_mime(raw: &str) -> Result<String, StorageError> {
    let mime: Mime = raw.parse().map_err(|_| StorageError::UnsupportedType)?;
    let value = mime.essence_str();
    if matches!(value, "image/jpeg" | "image/png" | "application/pdf") {
        Ok(value.to_owned())
    } else {
        Err(StorageError::UnsupportedType)
    }
}

fn validate_original_name(name: &str, mime_type: &str) -> Result<(), StorageError> {
    if name.is_empty()
        || name.len() > 255
        || name.contains('/')
        || name.contains('\\')
        || name == "."
        || name == ".."
        || name.chars().any(char::is_control)
    {
        return Err(StorageError::InvalidPath);
    }
    let extension = Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    let valid = match mime_type {
        "image/jpeg" => matches!(extension.as_deref(), Some("jpg" | "jpeg")),
        "image/png" => extension.as_deref() == Some("png"),
        "application/pdf" => extension.as_deref() == Some("pdf"),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(StorageError::UnsupportedType)
    }
}

fn extension_for(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => ".jpg",
        "image/png" => ".png",
        "application/pdf" => ".pdf",
        _ => "",
    }
}
