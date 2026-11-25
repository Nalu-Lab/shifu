//! Desktop OAuth returns to this one-use listener on the user's Mac.
use axum::{
    Router,
    extract::Query,
    http::{StatusCode, header},
    routing::get,
};
use serde::Deserialize;
use std::{
    future::IntoFuture,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpListener, sync::Notify};

#[derive(Deserialize)]
struct Callback {
    state: Option<String>,
    code: Option<String>,
    error: Option<String>,
}

pub(super) async fn receive(listener: TcpListener, state: String) -> Result<String, String> {
    let result = Arc::new(Mutex::new(None));
    let slot = result.clone();
    let done = Arc::new(Notify::new());
    let signal = done.clone();
    let router = Router::new().route(
        "/callback",
        get(move |Query(query): Query<Callback>| {
            let slot = slot.clone();
            let signal = signal.clone();
            let valid = query.state.as_deref() == Some(&state);
            async move {
                let headers = [
                    (header::CACHE_CONTROL, "no-store"),
                    (header::REFERRER_POLICY, "no-referrer"),
                ];
                if !valid {
                    return (
                        StatusCode::BAD_REQUEST,
                        headers,
                        "This sign-in request could not be matched.",
                    );
                }
                let code = query.code.filter(|s| !s.is_empty() && s.len() <= 2048);
                let value = match (code, query.error) {
                    (Some(code), None) => Ok(code),
                    _ => Err("google_cancelled".to_string()),
                };
                let mut result = slot.lock().expect("callback mutex");
                if result.is_none() {
                    *result = Some(value);
                    signal.notify_one();
                }
                (
                    StatusCode::OK,
                    headers,
                    "Return to Shifu to finish signing in. You can close this tab.",
                )
            }
        }),
    );
    let server =
        axum::serve(listener, router).with_graceful_shutdown(async move { done.notified().await });
    tokio::time::timeout(Duration::from_secs(240), server.into_future())
        .await
        .map_err(|_| "login_expired")?
        .map_err(|_| "callback_unavailable")?;
    result
        .lock()
        .expect("callback mutex")
        .take()
        .ok_or("callback_unavailable")?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unrelated_callbacks_leave_the_real_sign_in_waiting() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let url = format!("http://{}/callback", listener.local_addr().unwrap());
        let task = tokio::spawn(receive(listener, "expected".into()));
        let client = reqwest::Client::new();
        let wrong = client
            .get(format!("{url}?state=different&code=unused"))
            .send()
            .await
            .unwrap();
        assert_eq!(wrong.status(), StatusCode::BAD_REQUEST);
        let good = client
            .get(format!("{url}?state=expected&code=accepted"))
            .send()
            .await
            .unwrap();
        assert!(good.status().is_success());
        assert_eq!(task.await.unwrap().unwrap(), "accepted");
    }
}
