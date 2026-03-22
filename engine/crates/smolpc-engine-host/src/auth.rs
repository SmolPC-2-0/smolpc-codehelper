use axum::http::{HeaderMap, StatusCode};
use axum::Json;

use crate::types::{ApiError, ErrorResponse};

pub(crate) fn auth(headers: &HeaderMap, token: &str) -> Result<(), ApiError> {
    let Some(value) = headers.get("authorization") else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        ));
    };
    let Ok(value) = value.to_str() else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        ));
    };
    let expected = format!("Bearer {token}");
    if !constant_time_eq(value.as_bytes(), expected.as_bytes()) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        ));
    }
    Ok(())
}

pub(crate) fn constant_time_eq(lhs: &[u8], rhs: &[u8]) -> bool {
    let len_diff = (lhs.len() ^ rhs.len()) as u8;
    let min_len = lhs.len().min(rhs.len());
    let mut diff = len_diff;
    for i in 0..min_len {
        diff |= lhs[i] ^ rhs[i];
    }
    diff == 0
}
