use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum JsonRpcId {
    Number(u64),
    String(String),
    Null,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct JsonRpcRequest<T = serde_json::Value> {
    pub jsonrpc: String,
    pub id: JsonRpcId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<T>,
}

impl<T> JsonRpcRequest<T> {
    pub fn new(id: JsonRpcId, method: impl Into<String>, params: Option<T>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.into(),
            params,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct JsonRpcResponse<T = serde_json::Value> {
    pub jsonrpc: String,
    pub id: JsonRpcId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl<T> JsonRpcResponse<T> {
    pub fn success(id: JsonRpcId, result: T) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn failure(id: JsonRpcId, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse};

    #[test]
    fn jsonrpc_request_round_trips() {
        let request = JsonRpcRequest::new(
            JsonRpcId::Number(7),
            "tools/list",
            Some(serde_json::json!({ "cursor": null })),
        );

        let encoded = serde_json::to_string(&request).expect("serialize request");
        let decoded: JsonRpcRequest = serde_json::from_str(&encoded).expect("deserialize request");

        assert_eq!(decoded.method, "tools/list");
        assert_eq!(decoded.id, JsonRpcId::Number(7));
        assert_eq!(
            decoded.params.expect("params")["cursor"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn jsonrpc_response_round_trips() {
        let response = JsonRpcResponse::success(
            JsonRpcId::String("req-1".to_string()),
            serde_json::json!({
                "tools": []
            }),
        );

        let encoded = serde_json::to_string(&response).expect("serialize response");
        let decoded: JsonRpcResponse =
            serde_json::from_str(&encoded).expect("deserialize response");

        assert_eq!(decoded.id, JsonRpcId::String("req-1".to_string()));
        assert_eq!(
            decoded.result.expect("result")["tools"],
            serde_json::json!([])
        );
    }

    #[test]
    fn jsonrpc_error_response_round_trips() {
        let response = JsonRpcResponse::<serde_json::Value>::failure(
            JsonRpcId::Number(9),
            JsonRpcError {
                code: -32000,
                message: "transport unavailable".to_string(),
                data: Some(serde_json::json!({ "retryable": true })),
            },
        );

        let encoded = serde_json::to_string(&response).expect("serialize error response");
        let decoded: JsonRpcResponse =
            serde_json::from_str(&encoded).expect("deserialize error response");

        let error = decoded.error.expect("error");
        assert_eq!(error.code, -32000);
        assert_eq!(error.message, "transport unavailable");
        assert_eq!(error.data.expect("data")["retryable"], true);
    }
}
