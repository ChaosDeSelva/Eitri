//! Runner behaviour against a mock OpenAI server: a full run, budget stop, auth stop with
//! resume, pause and resume, and the automatic background fix.

use base64::Engine;
use eitri_lib::runner::{Notify, RunState, Runner};
use eitri_lib::store::{Item, ProjectPatch, Store};
use image::{Rgba, RgbaImage};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Default)]
struct Recorder {
    runs: Mutex<Vec<RunState>>,
}
impl Notify for Recorder {
    fn item(&self, _: &Item) {}
    fn run(&self, _: &str, s: &RunState) {
        self.runs.lock().unwrap().push(s.clone());
    }
    fn spend(&self, _: &str, _: f64) {}
}

fn png_b64(img: &RgbaImage) -> String {
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img.clone())
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    base64::engine::general_purpose::STANDARD.encode(out)
}

fn bust() -> RgbaImage {
    let mut img = RgbaImage::from_pixel(256, 256, Rgba([0, 0, 0, 0]));
    for y in 0..256 {
        for x in 0..256 {
            if y >= 150 || ((88..168).contains(&x) && (30..150).contains(&y)) {
                img.put_pixel(x, y, Rgba([180, 120, 90, 255]));
            }
        }
    }
    img
}

fn image_body(img: &RgbaImage) -> serde_json::Value {
    json!({"data": [{"b64_json": png_b64(img)}],
           "usage": {"input_tokens": 100, "output_tokens": 1000,
                     "input_tokens_details": {"text_tokens": 100, "image_tokens": 0}}})
}

struct Fixture {
    store: Arc<Store>,
    runner: Arc<Runner>,
    project: String,
    _dir: tempfile::TempDir,
}

async fn fixture(server: &MockServer, items: usize, gate: &str) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(dir.path()).unwrap());
    store.set_setting("api_base", &server.uri()).unwrap();
    let p = store
        .create_project("Test", "gpt-image-1", "gpt-4.1-mini", 2, 0.0)
        .unwrap();
    store
        .update_project(
            &p.id,
            ProjectPatch {
                gate: Some(gate.into()),
                shared_tail: Some("flat style".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let plan: Vec<(String, String, String)> = (0..items)
        .map(|i| {
            (
                format!("item-{i}"),
                format!("Item {i}"),
                format!("Subject {i}"),
            )
        })
        .collect();
    store.replace_plan(&p.id, &plan).unwrap();
    store.lock_spec(&p.id, true).unwrap();
    let runner = Runner::new(
        store.clone(),
        Arc::new(Recorder::default()),
        Arc::new(|_: &str| Ok(Some("sk-test-key-0000000000".into()))),
    );
    Fixture {
        store,
        runner,
        project: p.id,
        _dir: dir,
    }
}

async fn wait_until_not_running(runner: &Runner, pid: &str) -> RunState {
    let t = Instant::now();
    loop {
        let s = runner.state(pid);
        if matches!(s, RunState::Idle | RunState::Stopped(_)) {
            return s;
        }
        assert!(
            t.elapsed() < Duration::from_secs(20),
            "run did not finish: {s:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn full_run_draws_everything_and_records_cost() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/images/generations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(image_body(&bust())))
        .mount(&server)
        .await;
    let f = fixture(&server, 5, "portrait").await;
    f.store.enqueue(&f.project, None, 0).unwrap();
    f.runner.start(&f.project).unwrap();
    assert_eq!(
        wait_until_not_running(&f.runner, &f.project).await,
        RunState::Idle
    );
    let stats = f.store.stats(&f.project).unwrap();
    assert_eq!(stats.done, 5, "{stats:?}");
    // 100 text-in at $5/M + 1000 out at $40/M = $0.0405 per image.
    assert!(
        (stats.spent_usd - 5.0 * 0.0405).abs() < 1e-9,
        "{}",
        stats.spent_usd
    );
    let item = &f.store.items(&f.project).unwrap()[0];
    assert!(std::path::Path::new(item.file.as_ref().unwrap()).exists());
}

#[tokio::test]
async fn budget_cap_stops_the_run() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/images/generations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(image_body(&bust())))
        .mount(&server)
        .await;
    let f = fixture(&server, 10, "portrait").await;
    f.store
        .update_project(
            &f.project,
            ProjectPatch {
                budget_usd: Some(0.1),
                concurrency: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
    f.store.enqueue(&f.project, None, 0).unwrap();
    f.runner.start(&f.project).unwrap();
    let end = wait_until_not_running(&f.runner, &f.project).await;
    assert!(
        matches!(end, RunState::Stopped(ref r) if r.contains("Budget")),
        "{end:?}"
    );
    let stats = f.store.stats(&f.project).unwrap();
    assert_eq!(stats.done, 3, "three images at $0.0405 cross $0.10");
    assert_eq!(stats.queued, 7);
}

#[tokio::test]
async fn expired_key_stops_and_leaves_the_item_queued() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/images/generations"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"error": {"code": "expired_secret_key", "message": "Your API key has expired."}})))
        .mount(&server).await;
    let f = fixture(&server, 4, "portrait").await;
    f.store
        .update_project(
            &f.project,
            ProjectPatch {
                concurrency: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
    f.store.enqueue(&f.project, None, 0).unwrap();
    f.runner.start(&f.project).unwrap();
    let end = wait_until_not_running(&f.runner, &f.project).await;
    assert!(
        matches!(end, RunState::Stopped(ref r) if r.contains("expired")),
        "{end:?}"
    );
    let stats = f.store.stats(&f.project).unwrap();
    assert_eq!((stats.queued, stats.done, stats.failed), (4, 0, 0));
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        1,
        "no retries, no polling"
    );
}

#[tokio::test]
async fn pause_holds_the_queue_and_resume_finishes() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/images/generations"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(image_body(&bust()))
                .set_delay(Duration::from_millis(300)),
        )
        .mount(&server)
        .await;
    let f = fixture(&server, 6, "portrait").await;
    f.store
        .update_project(
            &f.project,
            ProjectPatch {
                concurrency: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
    f.store.enqueue(&f.project, None, 0).unwrap();
    f.runner.start(&f.project).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(f.runner.pause(&f.project), RunState::Paused);
    tokio::time::sleep(Duration::from_millis(900)).await;
    let paused = f.store.stats(&f.project).unwrap();
    assert!(paused.done <= 1 && paused.queued >= 5, "{paused:?}");
    f.runner.start(&f.project).unwrap();
    assert_eq!(
        wait_until_not_running(&f.runner, &f.project).await,
        RunState::Idle
    );
    assert_eq!(f.store.stats(&f.project).unwrap().done, 6);
}

#[tokio::test]
async fn solid_background_gets_one_automatic_fix() {
    let server = MockServer::start().await;
    let solid = RgbaImage::from_pixel(256, 256, Rgba([255, 255, 255, 255]));
    Mock::given(method("POST"))
        .and(path("/images/generations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(image_body(&solid)))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/images/edits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(image_body(&bust())))
        .expect(1)
        .mount(&server)
        .await;
    let f = fixture(&server, 1, "portrait").await;
    f.store.enqueue(&f.project, None, 0).unwrap();
    f.runner.start(&f.project).unwrap();
    assert_eq!(
        wait_until_not_running(&f.runner, &f.project).await,
        RunState::Idle
    );
    let item = &f.store.items(&f.project).unwrap()[0];
    assert_eq!(item.status, "done");
    let attempts = f.store.attempts(&item.id).unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].kind, "edit");
    assert_eq!(attempts[1].verdict.as_deref(), Some("fail"));
}

#[tokio::test]
async fn per_image_provider_with_keyed_background() {
    let server = MockServer::start().await;
    // A figure on a flat chroma backdrop, as a provider without alpha returns it.
    let mut green = RgbaImage::from_pixel(256, 256, Rgba([0, 255, 0, 255]));
    for y in 60..200 {
        for x in 90..166 {
            green.put_pixel(x, y, Rgba([120, 70, 200, 255]));
        }
    }
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(green)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    Mock::given(method("POST"))
        .and(path("/fal-ai/flux/dev"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"images": [{"url": format!("{}/out.png", server.uri())}]})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/out.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(png))
        .mount(&server)
        .await;

    let f = fixture(&server, 1, "figure").await;
    f.store.set_setting("base_url.fal", &server.uri()).unwrap();
    f.store
        .update_project(
            &f.project,
            ProjectPatch {
                provider: Some("fal".into()),
                image_model: Some("fal-ai/flux/dev".into()),
                background: Some("transparent".into()),
                ..Default::default()
            },
        )
        .unwrap();
    f.store.enqueue(&f.project, None, 0).unwrap();
    f.runner.start(&f.project).unwrap();
    assert_eq!(
        wait_until_not_running(&f.runner, &f.project).await,
        RunState::Idle
    );

    let item = &f.store.items(&f.project).unwrap()[0];
    assert_eq!(
        item.status, "done",
        "{:?} {:?}",
        item.reasons, item.last_error
    );
    let out = image::open(item.file.as_ref().unwrap()).unwrap().to_rgba8();
    assert_eq!(out.get_pixel(3, 3)[3], 0, "backdrop keyed out");
    assert_eq!(out.get_pixel(128, 128)[3], 255, "subject kept");
    let attempt = &f.store.attempts(&item.id).unwrap()[0];
    assert!(attempt.prompt.contains("chroma-key green"));
    assert!(
        (attempt.cost - 0.025).abs() < 1e-9,
        "per-image estimate, got {}",
        attempt.cost
    );
}
