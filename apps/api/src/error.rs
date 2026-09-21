use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use ownstate_services::ServiceError;
use serde_json::json;

/// HTTP-facing error. Internal detail is logged server-side; response bodies
/// carry only a stable code and a safe message.
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "invalid_request",
            message: message.into(),
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: message.into(),
        }
    }
}

impl From<ServiceError> for ApiError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::NotFound(entity) => ApiError {
                status: StatusCode::NOT_FOUND,
                code: "not_found",
                message: format!("{entity} not found"),
            },
            ServiceError::Validation(msg) => ApiError {
                status: StatusCode::BAD_REQUEST,
                code: "invalid_request",
                message: msg,
            },
            ServiceError::Conflict(msg) => ApiError {
                status: StatusCode::CONFLICT,
                code: "conflict",
                message: msg,
            },
            ServiceError::Forbidden(msg) => ApiError {
                status: StatusCode::FORBIDDEN,
                code: "forbidden",
                message: msg,
            },
            ServiceError::Embedding(msg) => {
                tracing::error!(error = %msg, "embedding failure");
                ApiError {
                    status: StatusCode::SERVICE_UNAVAILABLE,
                    code: "embedding_unavailable",
                    message: "embedding provider unavailable".into(),
                }
            }
            ServiceError::Internal(source) => {
                tracing::error!(error = %source, "internal service error");
                ApiError {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    code: "internal",
                    message: "internal error".into(),
                }
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "error": { "code": self.code, "message": self.message }
        }));
        (self.status, body).into_response()
    }
}
