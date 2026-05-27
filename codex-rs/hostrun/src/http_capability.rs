use std::fs;
use std::time::Duration;

use reqwest::Method;
use reqwest::blocking::Client;
use reqwest::blocking::RequestBuilder;
use reqwest::blocking::Response;
use reqwest::header::ACCEPT;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use reqwest::redirect::Policy;
use serde_json::Value;
use serde_json::json;

use super::HostrunApprovalRequest;
use super::HostrunSessionError;

pub(super) fn http_request_approval(args: Value) -> HostrunApprovalRequest {
    let method = field_as_string(&args, "method");
    let url = field_as_string(&args, "url");
    HostrunApprovalRequest {
        id: format!("http.request:{}:{url}", method.to_uppercase()),
        tool: "http.request".to_string(),
        summary: format!("HTTP {} {url}", method.to_uppercase()),
        args: redact_http_auth(args),
    }
}

pub(super) fn execute_http_request(args: Value) -> Result<Value, HostrunSessionError> {
    let method = parse_method(&args)?;
    let url = field_as_string(&args, "url");
    let client = build_client(&args)?;
    let request = client
        .request(method, &url)
        .headers(headers_from_args(&args)?);
    let request = apply_query(request, args.get("query"))?;
    let request = apply_auth(request, args.get("auth"))?;
    let request = apply_body(request, &args)?;
    let response = send_with_retries(request, retry_count(&args)).map_err(|error| {
        HostrunSessionError::Eval(format!("HTTP request failed for {url}: {error}"))
    })?;
    let status = response.status();
    let headers = response_headers(response.headers());
    let body = response.bytes().map_err(|error| {
        HostrunSessionError::Eval(format!("failed to read HTTP response body: {error}"))
    })?;
    if should_throw_on_status(status.as_u16(), &args) {
        return Err(HostrunSessionError::Eval(format!(
            "HTTP request failed with status {} for {url}: {}",
            status.as_u16(),
            String::from_utf8_lossy(&body)
        )));
    }
    response_value(status.as_u16(), headers, &body, args.get("response"))
}

fn build_client(args: &Value) -> Result<Client, HostrunSessionError> {
    let mut builder = Client::builder();
    if let Some(timeout) = args.get("timeout").and_then(parse_duration) {
        builder = builder.timeout(timeout);
    }
    if let Some(policy) = redirect_policy(args.get("redirects").or_else(|| args.get("redirect"))) {
        builder = builder.redirect(policy);
    }
    if tls_accept_invalid_certs(args.get("tls")) {
        builder = builder.danger_accept_invalid_certs(true);
    }
    builder
        .build()
        .map_err(|error| HostrunSessionError::Eval(format!("failed to build HTTP client: {error}")))
}

fn redirect_policy(value: Option<&Value>) -> Option<Policy> {
    match value {
        Some(Value::Bool(false)) => Some(Policy::none()),
        Some(Value::Number(limit)) => limit.as_u64().map(|limit| Policy::limited(limit as usize)),
        Some(Value::Object(options)) => {
            if options.get("enabled") == Some(&Value::Bool(false)) {
                return Some(Policy::none());
            }
            options
                .get("limit")
                .and_then(Value::as_u64)
                .map(|limit| Policy::limited(limit as usize))
        }
        _ => None,
    }
}

fn tls_accept_invalid_certs(value: Option<&Value>) -> bool {
    value
        .and_then(|value| value.get("acceptInvalidCerts"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn retry_count(args: &Value) -> u64 {
    args.get("retries").and_then(Value::as_u64).unwrap_or(0)
}

fn send_with_retries(request: RequestBuilder, retries: u64) -> reqwest::Result<Response> {
    let mut attempts = 0;
    let mut request = request;
    loop {
        let Some(next_request) = request.try_clone() else {
            return request.send();
        };
        match request.send() {
            Ok(response) => return Ok(response),
            Err(error) if attempts < retries => {
                attempts += 1;
                request = next_request;
                if !error.is_timeout() && !error.is_connect() {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    }
}

fn should_throw_on_status(status: u16, args: &Value) -> bool {
    args.get("throwOnError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && !(200..300).contains(&status)
}

fn parse_method(args: &Value) -> Result<Method, HostrunSessionError> {
    field_as_string(args, "method")
        .parse()
        .map_err(|error| HostrunSessionError::Eval(format!("invalid HTTP method: {error}")))
}

fn headers_from_args(args: &Value) -> Result<HeaderMap, HostrunSessionError> {
    let mut headers = HeaderMap::new();
    if let Some(Value::Object(values)) = args.get("headers") {
        for (name, value) in values {
            let name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                HostrunSessionError::Eval(format!("invalid HTTP header name {name}: {error}"))
            })?;
            let value = HeaderValue::from_str(&value_to_string(value)).map_err(|error| {
                HostrunSessionError::Eval(format!("invalid HTTP header value for {name}: {error}"))
            })?;
            headers.insert(name, value);
        }
    }
    if args.get("json").is_some() && !headers.contains_key(ACCEPT) {
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    }
    Ok(headers)
}

fn apply_query(
    request: reqwest::blocking::RequestBuilder,
    query: Option<&Value>,
) -> Result<reqwest::blocking::RequestBuilder, HostrunSessionError> {
    let Some(Value::Object(query)) = query else {
        return Ok(request);
    };
    let pairs = query
        .iter()
        .map(|(key, value)| (key.as_str(), value_to_string(value)))
        .collect::<Vec<_>>();
    Ok(request.query(&pairs))
}

fn apply_auth(
    request: reqwest::blocking::RequestBuilder,
    auth: Option<&Value>,
) -> Result<reqwest::blocking::RequestBuilder, HostrunSessionError> {
    let Some(Value::Object(auth)) = auth else {
        return Ok(request);
    };
    if let Some(token) = auth.get("bearer").and_then(Value::as_str) {
        return Ok(request.bearer_auth(token));
    }
    if let Some(Value::Object(basic)) = auth.get("basic") {
        let username = basic.get("username").and_then(Value::as_str).unwrap_or("");
        let password = basic.get("password").and_then(Value::as_str);
        return Ok(request.basic_auth(username, password));
    }
    Ok(request)
}

fn apply_body(
    request: reqwest::blocking::RequestBuilder,
    args: &Value,
) -> Result<reqwest::blocking::RequestBuilder, HostrunSessionError> {
    if let Some(value) = args.get("json") {
        return Ok(request.json(value));
    }
    if let Some(Value::Object(form)) = args.get("form") {
        return Ok(request.form(form));
    }
    if let Some(value) = args.get("body") {
        return Ok(request.body(body_bytes(value)?));
    }
    if let Some(path) = args.get("file").and_then(Value::as_str) {
        let bytes = fs::read(path).map_err(|error| {
            HostrunSessionError::Eval(format!("failed to read HTTP request file {path}: {error}"))
        })?;
        return Ok(request.body(bytes));
    }
    if args.get("multipart").is_some() {
        return Err(HostrunSessionError::Eval(
            "HTTP multipart execution is not implemented yet".to_string(),
        ));
    }
    Ok(request)
}

fn body_bytes(value: &Value) -> Result<Vec<u8>, HostrunSessionError> {
    match value {
        Value::Array(values) => values.iter().map(body_byte).collect::<Result<Vec<_>, _>>(),
        _ => Ok(value_to_string(value).into_bytes()),
    }
}

fn body_byte(value: &Value) -> Result<u8, HostrunSessionError> {
    let byte = value.as_u64().ok_or_else(|| {
        HostrunSessionError::Eval("HTTP byte bodies must contain byte numbers".to_string())
    })?;
    u8::try_from(byte).map_err(|_| {
        HostrunSessionError::Eval(format!("HTTP byte body value is out of range: {byte}"))
    })
}

fn response_value(
    status: u16,
    headers: Value,
    body: &[u8],
    response: Option<&Value>,
) -> Result<Value, HostrunSessionError> {
    let response_type = response
        .and_then(|response| response.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("text");
    let mut output = serde_json::Map::new();
    output.insert("status".to_string(), json!(status));
    output.insert("ok".to_string(), Value::Bool((200..300).contains(&status)));
    output.insert("headers".to_string(), headers);
    output.insert("bytes".to_string(), json!(body.len()));
    apply_response_body(&mut output, response_type, response, body)?;
    Ok(Value::Object(output))
}

fn apply_response_body(
    output: &mut serde_json::Map<String, Value>,
    response_type: &str,
    response: Option<&Value>,
    body: &[u8],
) -> Result<(), HostrunSessionError> {
    match response_type {
        "run" => {}
        "text" => {
            output.insert(
                "text".to_string(),
                Value::String(String::from_utf8_lossy(body).to_string()),
            );
        }
        "json" => {
            let value = serde_json::from_slice(body).map_err(|error| {
                HostrunSessionError::Eval(format!("failed to parse HTTP response JSON: {error}"))
            })?;
            output.insert("json".to_string(), value);
        }
        "bytes" => {
            output.insert("body".to_string(), json!(body));
        }
        "file" => {
            let path = response
                .and_then(|response| response.get("path"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            fs::write(path, body).map_err(|error| {
                HostrunSessionError::Eval(format!(
                    "failed to save HTTP response to {path}: {error}"
                ))
            })?;
            output.insert("path".to_string(), Value::String(path.to_string()));
        }
        other => {
            return Err(HostrunSessionError::Eval(format!(
                "unsupported HTTP response type: {other}"
            )));
        }
    }
    Ok(())
}

fn response_headers(headers: &HeaderMap) -> Value {
    Value::Object(
        headers
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_string(),
                    Value::String(value.to_str().unwrap_or("").to_string()),
                )
            })
            .collect(),
    )
}

fn parse_duration(value: &Value) -> Option<Duration> {
    if let Some(seconds) = value.as_u64() {
        return Some(Duration::from_secs(seconds));
    }
    let text = value.as_str()?;
    if let Some(ms) = text.strip_suffix("ms").and_then(|value| value.parse().ok()) {
        return Some(Duration::from_millis(ms));
    }
    text.strip_suffix('s')
        .and_then(|value| value.parse().ok())
        .map(Duration::from_secs)
}

fn redact_http_auth(mut args: Value) -> Value {
    redact_http_auth_field(&mut args);
    redact_http_headers(&mut args);
    args
}

fn redact_http_auth_field(args: &mut Value) {
    let Some(auth) = args.get_mut("auth") else {
        return;
    };
    match auth {
        Value::Object(auth) => {
            for key in ["bearer", "token"] {
                redact_object_key(auth, key);
            }
            if let Some(basic) = auth.get_mut("basic") {
                redact_http_basic_auth(basic);
            }
        }
        other => {
            *other = Value::String("<redacted>".to_string());
        }
    }
}

fn redact_http_basic_auth(basic: &mut Value) {
    match basic {
        Value::Object(basic) => redact_object_key(basic, "password"),
        other => *other = Value::String("<redacted>".to_string()),
    }
}

fn redact_http_headers(args: &mut Value) {
    let Some(Value::Object(headers)) = args.get_mut("headers") else {
        return;
    };
    for (key, value) in headers {
        if is_sensitive_http_header(key) {
            *value = Value::String("<redacted>".to_string());
        }
    }
}

fn is_sensitive_http_header(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "authorization" | "proxy-authorization" | "x-api-key" | "x-auth-token"
    )
}

fn redact_object_key(object: &mut serde_json::Map<String, Value>, key: &str) {
    if object.contains_key(key) {
        object.insert(key.to_string(), Value::String("<redacted>".to_string()));
    }
}

fn field_as_string(args: &Value, field: &str) -> String {
    args.get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => String::new(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}
