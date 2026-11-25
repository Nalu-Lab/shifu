//! The account directory is optional. Its credentials never enter the webview or the remote relay.
mod browser;

use reqwest::{Client, Method, Response, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tauri::State;
use tokio::net::TcpListener;
use uuid::Uuid;

pub fn configured() -> Option<String> {
    std::env::var("SHIFU_ACCOUNTS_URL")
        .ok()
        .or_else(|| option_env!("SHIFU_ACCOUNTS_URL").map(str::to_owned))
        .filter(|s| !s.is_empty())
}

fn origin(value: &str) -> Result<String, String> {
    let url = Url::parse(value).map_err(|_| "invalid_service")?;
    let local = matches!(url.host_str(), Some("127.0.0.1" | "[::1]"));
    if !(url.scheme() == "https" || (local && url.scheme() == "http"))
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("invalid_service".into());
    }
    Ok(url.origin().ascii_serialization())
}

#[derive(Deserialize, Serialize)]
struct Local {
    device: String,
    session: Option<String>,
}

impl Default for Local {
    fn default() -> Self {
        Self {
            device: Uuid::new_v4().simple().to_string(),
            session: None,
        }
    }
}

#[cfg(target_os = "macos")]
mod vault {
    use super::Local;
    use security_framework::passwords::{get_generic_password, set_generic_password};

    fn service() -> String {
        format!(
            "{}.accounts",
            crate::config::running_bundle_id().unwrap_or_else(|| "dev.longado.shifu.dev".into())
        )
    }

    pub fn read(origin: &str) -> Result<Local, String> {
        match get_generic_password(&service(), origin) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "keychain_failed".into()),
            Err(e) if e.code() == -25300 => Ok(Local::default()), // errSecItemNotFound
            Err(_) => Err("keychain_failed".into()),
        }
    }

    pub fn save(origin: &str, local: &Local) -> Result<(), String> {
        let bytes = serde_json::to_vec(local).map_err(|_| "keychain_failed")?;
        set_generic_password(&service(), origin, &bytes).map_err(|_| "keychain_failed".into())
    }
}

#[cfg(not(target_os = "macos"))]
mod vault {
    use super::Local;
    pub fn read(_: &str) -> Result<Local, String> {
        Err("keychain_failed".into())
    }
    pub fn save(_: &str, _: &Local) -> Result<(), String> {
        Err("keychain_failed".into())
    }
}

struct Admission {
    origin: String,
    ticket: String,
    deadline: Instant,
}

#[derive(Default)]
pub struct Accounts(tokio::sync::Mutex<Option<Admission>>);

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    Refresh,
    SignIn { name: String },
    Redeem { code: String },
    Cancel,
    SignOut,
    Remove { id: String },
}

#[derive(Deserialize, Serialize)]
pub struct Mac {
    id: String,
    name: String,
}

#[derive(Deserialize, Serialize)]
pub struct Directory {
    email: String,
    current: String,
    devices: Vec<Mac>,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum View {
    SignedOut,
    InviteRequired,
    SignedIn(Directory),
}

#[derive(Deserialize)]
struct Start {
    state: String,
    flow: String,
    url: String,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Login {
    InviteRequired { ticket: String },
    SignedIn { session: String },
}

struct Api {
    client: Client,
    origin: String,
}

impl Api {
    async fn request(
        &self,
        method: Method,
        path: &str,
        session: Option<&str>,
        body: Option<Value>,
    ) -> Result<Response, String> {
        let mut req = self
            .client
            .request(method, format!("{}{path}", self.origin));
        if let Some(token) = session {
            req = req.bearer_auth(token);
        }
        if let Some(value) = body {
            req = req.json(&value);
        }
        let response = req.send().await.map_err(|_| "service_unavailable")?;
        if response.status().is_success() {
            return Ok(response);
        }
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err("rate_limited".into());
        }
        let data = response.json::<Value>().await.unwrap_or_default();
        Err(data["error"]
            .as_str()
            .unwrap_or("service_unavailable")
            .to_owned())
    }

    async fn google(
        &self,
        device: &str,
        name: &str,
        open: impl FnOnce(&Url) -> Result<(), String>,
    ) -> Result<Login, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|_| "callback_unavailable")?;
        let port = listener
            .local_addr()
            .map_err(|_| "callback_unavailable")?
            .port();
        let start: Start = self
            .request(
                Method::POST,
                "/v1/login/start",
                None,
                Some(json!({ "port": port, "device": device, "name": name })),
            )
            .await?
            .json()
            .await
            .map_err(|_| "invalid_service")?;
        let url = Url::parse(&start.url).map_err(|_| "invalid_service")?;
        let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
        let matches = |key: &str, expected: &str| params.get(key).is_some_and(|v| v == expected);
        if url.origin().ascii_serialization() != "https://accounts.google.com"
            || url.path() != "/o/oauth2/v2/auth"
            || !url.username().is_empty()
            || url.password().is_some()
            || start.state.len() != 43
            || !matches("state", &start.state)
            || !matches("code_challenge_method", "S256")
            || !matches("redirect_uri", &format!("http://127.0.0.1:{port}/callback"))
        {
            return Err("invalid_service".into());
        }
        open(&url)?;
        let code = browser::receive(listener, start.state).await?;
        self.request(
            Method::POST,
            "/v1/login/finish",
            None,
            Some(json!({ "flow": start.flow, "code": code })),
        )
        .await?
        .json()
        .await
        .map_err(|_| "invalid_service".into())
    }

    async fn directory(&self, local: &mut Local) -> Result<View, String> {
        let Some(session) = local.session.as_deref() else {
            return Ok(View::SignedOut);
        };
        match self
            .request(Method::GET, "/v1/me", Some(session), None)
            .await
        {
            Ok(response) => Ok(View::SignedIn(
                response.json().await.map_err(|_| "invalid_service")?,
            )),
            Err(code) if code == "signed_out" => {
                local.session = None;
                vault::save(&self.origin, local)?;
                Ok(View::SignedOut)
            }
            Err(code) => Err(code),
        }
    }
}

fn open_browser(url: &Url) -> Result<(), String> {
    let status = std::process::Command::new("/usr/bin/open")
        .arg(url.as_str())
        .status()
        .map_err(|_| "browser_failed")?;
    if status.success() {
        Ok(())
    } else {
        Err("browser_failed".into())
    }
}

#[tauri::command]
pub async fn account_action(action: Action, accounts: State<'_, Accounts>) -> Result<View, String> {
    let origin = origin(&configured().ok_or("invalid_service")?)?;
    let mut pending = accounts.0.try_lock().map_err(|_| "sign_in_busy")?;
    if pending
        .as_ref()
        .is_some_and(|p| p.origin != origin || Instant::now() >= p.deadline)
    {
        *pending = None;
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "service_unavailable")?;
    let api = Api { client, origin };
    let mut local = vault::read(&api.origin)?;
    let login = match action {
        Action::SignIn { name } => {
            *pending = None;
            vault::save(&api.origin, &local)?;
            Some(api.google(&local.device, name.trim(), open_browser).await?)
        }
        Action::Redeem { code } => {
            let admission = pending.as_ref().ok_or("login_expired")?;
            let result = api
                .request(
                    Method::POST,
                    "/v1/login/invite",
                    None,
                    Some(json!({ "ticket": admission.ticket, "code": code })),
                )
                .await;
            if result
                .as_ref()
                .err()
                .is_some_and(|err| err == "login_expired")
            {
                *pending = None;
            }
            Some(
                result?
                    .json::<Login>()
                    .await
                    .map_err(|_| "invalid_service")?,
            )
        }
        Action::Cancel => {
            *pending = None;
            None
        }
        Action::SignOut => {
            if let Some(token) = local.session.as_deref() {
                if let Err(err) = api
                    .request(Method::POST, "/v1/logout", Some(token), None)
                    .await
                    && err != "signed_out"
                {
                    return Err(err);
                }
            }
            local.session = None;
            vault::save(&api.origin, &local)?;
            *pending = None;
            return Ok(View::SignedOut);
        }
        Action::Remove { id } => {
            if id.len() != 32 || !id.bytes().all(|v| v.is_ascii_hexdigit()) {
                return Err("unknown_mac".into());
            }
            let token = local.session.as_deref().ok_or("signed_out")?;
            if let Err(err) = api
                .request(Method::DELETE, &format!("/v1/macs/{id}"), Some(token), None)
                .await
                && err != "signed_out"
            {
                return Err(err);
            }
            None
        }
        Action::Refresh => None,
    };
    match login {
        Some(Login::InviteRequired { ticket }) => {
            *pending = Some(Admission {
                origin: api.origin.clone(),
                ticket,
                deadline: Instant::now() + Duration::from_secs(600),
            });
        }
        Some(Login::SignedIn { session }) => {
            local.session = Some(session);
            vault::save(&api.origin, &local)?;
            *pending = None;
        }
        None => {}
    }
    if pending.is_some() {
        Ok(View::InviteRequired)
    } else {
        api.directory(&mut local).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokens_are_scoped_to_an_https_origin_or_an_explicit_loopback_service() {
        assert_eq!(
            origin("https://accounts.example/").unwrap(),
            "https://accounts.example"
        );
        assert_eq!(
            origin("http://127.0.0.1:3845").unwrap(),
            "http://127.0.0.1:3845"
        );
        for bad in [
            "http://example.com",
            "https://alice@example.com",
            "https://example.com/other",
            "file:///tmp/x",
        ] {
            assert!(origin(bad).is_err());
        }
    }
    #[tokio::test]
    async fn desktop_round_trip_returns_to_the_invite_gate() {
        use axum::{Router, http::header::CONTENT_TYPE, routing::post};
        use std::future::IntoFuture;

        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let api = Api {
            client: Client::new(),
            origin,
        };
        let router = Router::new()
            .route(
                "/v1/login/start",
                post(|raw: String| async move {
                    let body: Value = serde_json::from_str(&raw).unwrap();
                    assert_eq!(body["name"], "Studio");
                    let redirect = format!("http://127.0.0.1:{}/callback", body["port"]);
                    let mut url =
                        Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();
                    let state = "s".repeat(43);
                    url.query_pairs_mut()
                        .append_pair("state", &state)
                        .append_pair("code_challenge_method", "S256")
                        .append_pair("redirect_uri", &redirect);
                    (
                        [(CONTENT_TYPE, "application/json")],
                        json!({"state":state,"url":url.as_str(),"flow":"private-handoff"})
                            .to_string(),
                    )
                }),
            )
            .route(
                "/v1/login/finish",
                post(|raw: String| async move {
                    let body: Value = serde_json::from_str(&raw).unwrap();
                    assert_eq!(
                        body,
                        json!({"flow":"private-handoff","code":"from-browser"})
                    );
                    (
                        [(CONTENT_TYPE, "application/json")],
                        json!({"status":"invite_required","ticket":"pending-admission"})
                            .to_string(),
                    )
                }),
            );
        let server = tokio::spawn(axum::serve(listener, router).into_future());
        let result = api
            .google("test-device", "Studio", |url| {
                let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
                assert!(!url.as_str().contains("private-handoff"));
                let mut callback = Url::parse(&params["redirect_uri"]).unwrap();
                callback
                    .query_pairs_mut()
                    .append_pair("state", &params["state"])
                    .append_pair("code", "from-browser");
                tokio::spawn(async move {
                    reqwest::get(callback).await.unwrap();
                });
                Ok(())
            })
            .await
            .unwrap();
        server.abort();
        assert!(
            matches!(result, Login::InviteRequired { ticket } if ticket == "pending-admission")
        );
    }
}
