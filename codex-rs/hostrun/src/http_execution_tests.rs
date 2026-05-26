use std::fs;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::thread;

use serde_json::json;

use super::HostrunSession;

#[test]
fn approved_http_get_executes_query_headers_and_json_response() {
    let server = TestHttpServer::start(|request| {
        assert!(request.starts_with("get /users?"));
        assert!(request.contains("q=hostrun"));
        assert!(request.contains("limit=20"));
        assert!(request.contains("accept: application/json"));
        http_response("200 OK", "application/json", r#"{"ok":true,"count":2}"#)
    });
    let session = HostrunSession::new_auto_approve().expect("session");

    let result = session
        .eval(&format!(
            "http.get({}, {{
                query: {{ q: 'hostrun', limit: 20 }},
                headers: {{ Accept: 'application/json' }}
            }}).json();",
            json!(server.url("/users"))
        ))
        .expect("http get");

    assert_eq!(
        result.value,
        Some(json!({
            "status": 200,
            "ok": true,
            "headers": { "content-length": "21", "content-type": "application/json" },
            "bytes": 21,
            "json": { "ok": true, "count": 2 }
        }))
    );
}

#[test]
fn approved_http_post_sends_json_body_and_saves_response() {
    let server = TestHttpServer::start(|request| {
        assert!(request.starts_with("post /users "));
        assert!(request.contains("content-type: application/json"));
        assert!(request.ends_with(r#"{"name":"alice"}"#));
        http_response("201 Created", "application/json", r#"{"id":7}"#)
    });
    let session = HostrunSession::new_auto_approve().expect("session");
    let dir = tempfile::tempdir().expect("tempdir");
    let output = dir.path().join("user.json");
    let output_text = output.to_string_lossy().to_string();

    let result = session
        .eval(&format!(
            "http.post({}, {{ json: {{ name: 'Alice' }} }}).save({});",
            json!(server.url("/users")),
            json!(output_text)
        ))
        .expect("http post");

    assert_eq!(
        fs::read_to_string(&output).expect("saved response"),
        r#"{"id":7}"#
    );
    assert_eq!(
        result.value,
        Some(json!({
            "status": 201,
            "ok": true,
            "headers": { "content-length": "8", "content-type": "application/json" },
            "bytes": 8,
            "path": output_text
        }))
    );
}

struct TestHttpServer {
    url: String,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestHttpServer {
    fn start(handler: impl FnOnce(String) -> String + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let url = format!("http://{}", listener.local_addr().expect("local addr"));
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept connection");
            let request = read_http_request(&mut stream);
            let response = handler(request);
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });
        Self {
            url,
            handle: Some(handle),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.url, path)
    }
}

impl Drop for TestHttpServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0; 4096];
    loop {
        let count = stream.read(&mut chunk).expect("read request");
        if count == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..count]);
        let text = String::from_utf8_lossy(&buffer);
        if let Some(header_end) = text.find("\r\n\r\n") {
            let content_length = request_content_length(&text[..header_end]);
            let body_len = buffer.len().saturating_sub(header_end + 4);
            if body_len >= content_length {
                break;
            }
        }
    }
    String::from_utf8_lossy(&buffer).to_ascii_lowercase()
}

fn request_content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())
                .flatten()
        })
        .unwrap_or(0)
}

fn http_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\r\n{body}",
        body.len()
    )
}
