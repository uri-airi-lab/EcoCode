//! The client against a local mock backend.

use std::sync::mpsc;
use std::thread;

use ecocode_api::EcoCodeHttpClient;
use ecocode_api::types::SystemInfo;

/// Serves `responses` in order, sending each request's (method, url, auth, body) back.
fn mock(responses: Vec<(u16, &'static str)>) -> (String, mpsc::Receiver<(String, String, String, String)>) {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", server.server_addr().to_ip().unwrap());
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for (status, body) in responses {
            let mut req = server.recv().unwrap();
            let mut content = String::new();
            req.as_reader().read_to_string(&mut content).unwrap();
            let auth = req
                .headers()
                .iter()
                .find(|h| h.field.equiv("Authorization"))
                .map(|h| h.value.to_string())
                .unwrap_or_default();
            tx.send((req.method().to_string(), req.url().to_string(), auth, content)).unwrap();
            req.respond(tiny_http::Response::from_string(body).with_status_code(status)).unwrap();
        }
    });
    (url, rx)
}

#[test]
fn create_run_posts_payload_with_bearer_key() {
    let (url, rx) = mock(vec![(201, r#"{"id": 77}"#)]);
    let c = EcoCodeHttpClient::with_base_url(&url, "ecocode_testkey123").unwrap();
    let id = c.create_run(5, "r1", Some("ana"), Some("2026-01-01T00:00:00Z"), Some("python t.py"), "online").unwrap();
    assert_eq!(id, 77);
    let (method, path, auth, body) = rx.recv().unwrap();
    assert_eq!((method.as_str(), path.as_str()), ("POST", "/api/ingest/run"));
    assert_eq!(auth, "Bearer ecocode_testkey123");
    let body: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(body["project_id"], 5);
    assert_eq!(body["exporter_type"], "online");
    assert_eq!(body["collaborator_name"], "ana");
}

#[test]
fn system_info_joins_gpu_names() {
    let (url, rx) = mock(vec![(200, "")]);
    let c = EcoCodeHttpClient::with_base_url(&url, "k").unwrap();
    let info = SystemInfo {
        cpu_name: "cpu".into(),
        cpu_cores: 8,
        ram_gb: 16.0,
        gpu_count: 2,
        gpu_names: vec!["A".into(), "B".into()],
    };
    c.save_system_info(3, &info).unwrap();
    let (_, path, _, body) = rx.recv().unwrap();
    assert_eq!(path, "/api/ingest/run/3/system-info");
    assert!(body.contains(r#""gpu_names":"A, B""#), "{body}");
}

#[test]
fn errors_and_missing_config() {
    let (url, _rx) = mock(vec![(500, "boom"), (404, ""), (200, r#"[{"id":1,"name":"p"},{"id":2,"name":null}]"#)]);
    let c = EcoCodeHttpClient::with_base_url(&url, "k").unwrap();
    let err = c.list_projects().unwrap_err().to_string();
    assert!(err.contains("500") && err.contains("boom"), "{err}");
    assert!(c.carbon_config(1).unwrap().is_none());
    let projects = c.list_projects().unwrap();
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].name.as_deref(), Some("p"));
}
