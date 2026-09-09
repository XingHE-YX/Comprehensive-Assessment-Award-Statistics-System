pub mod admin;
mod category;
mod submission;
mod upload;

use std::{collections::BTreeMap, fmt};

pub use category::validate_category;
pub use submission::{
    SubmissionInput, ValidatedSubmission, validate_deadline, validate_declaration, validate_score,
    validate_student_update, validate_submission,
};
pub(crate) use upload::validate_uploads_with_existing;
pub use upload::{ValidatedUpload, validate_upload};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationErrors {
    pub fields: BTreeMap<String, Vec<String>>,
}

impl ValidationErrors {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.fields
            .entry(field.into())
            .or_default()
            .push(message.into());
    }

    pub fn extend(&mut self, other: Self) {
        for (field, messages) in other.fields {
            for message in messages {
                self.add(field.clone(), message);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn contains_key(&self, field: &str) -> bool {
        self.fields.contains_key(field)
    }

    pub fn first(&self, field: &str) -> Option<&str> {
        self.fields
            .get(field)
            .and_then(|messages| messages.first())
            .map(String::as_str)
    }
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for messages in self.fields.values() {
            for message in messages {
                if !first {
                    f.write_str("；")?;
                }
                first = false;
                f.write_str(message)?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for ValidationErrors {}

pub mod settings;
