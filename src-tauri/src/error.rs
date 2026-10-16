//! The single error type used by every command.
//!
//! Rust has no exceptions. Functions that can fail return `Result<T, E>`:
//! either `Ok(value)` or `Err(error)`. `AppError` is our `E`, and the `?`
//! operator lets a function bail out early with the error converted via `From`.

use serde::Serialize;
use specta::Type;
use thiserror::Error;

/// Every error the backend can report to the frontend.
///
/// - `thiserror::Error` writes the `Display` impl from the `#[error("...")]`
///   strings so we never hand-write `impl Display`.
/// - `Serialize` + `Type` make it crossable to TypeScript. With serde's
///   adjacent tagging it becomes `{ "kind": "validation", "message": "..." }`,
///   which is the shape CLAUDE.md asks for.
/// - Each variant carries a `String` rather than the original error (such as
///   `std::io::Error`) because those originals are not serialisable.
#[derive(Debug, Error, Serialize, Type)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(String),
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("{0}")]
    Internal(String),
}

/// Shorthand so command and service signatures read `AppResult<Foo>`.
pub type AppResult<T> = Result<T, AppError>;

/// Lets `?` turn an `std::io::Error` into an `AppError` automatically.
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err.to_string())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialises_as_kind_and_message() {
        let err = AppError::Validation("name is required".into());
        let value = serde_json::to_value(&err).expect("serialisable");
        assert_eq!(
            value,
            json!({ "kind": "validation", "message": "name is required" })
        );
    }

    #[test]
    fn multi_word_variants_are_camel_case() {
        let err = AppError::NotFound("entity 42".into());
        let value = serde_json::to_value(&err).expect("serialisable");
        assert_eq!(value["kind"], "notFound");
    }

    #[test]
    fn io_errors_convert_with_question_mark() {
        fn read_missing() -> AppResult<String> {
            // `?` converts std::io::Error -> AppError via the From impl above.
            let text = std::fs::read_to_string("definitely-missing-file.txt")?;
            Ok(text)
        }
        assert!(matches!(read_missing(), Err(AppError::Io(_))));
    }
}
