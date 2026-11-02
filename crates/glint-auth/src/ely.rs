//! Ely.by sign-in through its Yggdrasil-compatible auth server.
//!
//! The player's password goes only to Ely.by; Glint keeps the returned session
//! tokens in the system credential store and refreshes them before each launch.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{Error, Result};

pub const AUTH_SERVER: &str = "https://authserver.ely.by";

/// Argument for `-javaagent:authlib-injector.jar=<target>` so the game uses Ely.by.
pub const AUTHLIB_INJECTOR_TARGET: &str = "ely.by";

/// A signed-in Ely.by session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub client_token: String,
    /// Undashed profile UUID.
    pub uuid: String,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthResponse {
    access_token: String,
    client_token: String,
    selected_profile: Option<Profile>,
}

#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    error: Option<String>,
    error_message: Option<String>,
}

pub struct ElyClient {
    http: reqwest::Client,
    base: String,
}

impl ElyClient {
    pub fn new(http: reqwest::Client) -> Self {
        Self::with_base(http, AUTH_SERVER)
    }

    /// Points at another server; used by tests.
    pub fn with_base(http: reqwest::Client, base: impl Into<String>) -> Self {
        Self {
            http,
            base: base.into().trim_end_matches('/').to_owned(),
        }
    }

    /// Signs in with a nickname or email. Pass the code from the authenticator app as
    /// `totp` when the first attempt returns [`Error::TwoFactorRequired`].
    pub async fn authenticate(
        &self,
        login: &str,
        password: &str,
        totp: Option<&str>,
        client_token: &str,
    ) -> Result<Session> {
        let password = match totp.map(str::trim).filter(|c| !c.is_empty()) {
            Some(code) => format!("{password}:{code}"),
            None => password.to_owned(),
        };
        let body = json!({
            "username": login.trim(),
            "password": password,
            "clientToken": client_token,
            "requestUser": true,
        });
        self.session("/auth/authenticate", body).await
    }

    /// Exchanges a session for a fresh one. Fails with [`Error::SessionExpired`]
    /// when the player has to sign in again.
    pub async fn refresh(&self, session: &Session) -> Result<Session> {
        let body = json!({
            "accessToken": session.access_token,
            "clientToken": session.client_token,
            "requestUser": true,
        });
        match self.session("/auth/refresh", body).await {
            Err(Error::Rejected(_)) => Err(Error::SessionExpired),
            other => other.map(|s| Session {
                uuid: session.uuid.clone(),
                ..s
            }),
        }
    }

    /// Whether the access token is still accepted as is.
    pub async fn validate(&self, session: &Session) -> Result<bool> {
        let response = self
            .http
            .post(format!("{}/auth/validate", self.base))
            .json(&json!({ "accessToken": session.access_token }))
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok(response.status().is_success())
    }

    /// A valid session: the given one if still accepted, otherwise a refreshed one.
    pub async fn ensure_valid(&self, session: Session) -> Result<Session> {
        if self.validate(&session).await? {
            Ok(session)
        } else {
            self.refresh(&session).await
        }
    }

    /// Ends the session on the server. Failures are ignored by callers removing an account.
    pub async fn invalidate(&self, session: &Session) -> Result<()> {
        self.http
            .post(format!("{}/auth/invalidate", self.base))
            .json(&json!({ "accessToken": session.access_token, "clientToken": session.client_token }))
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok(())
    }

    async fn session(&self, path: &str, body: serde_json::Value) -> Result<Session> {
        let response = self
            .http
            .post(format!("{}{path}", self.base))
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        if !status.is_success() {
            let parsed: Option<ErrorResponse> = serde_json::from_str(&text).ok();
            let message = parsed
                .as_ref()
                .and_then(|e| e.error_message.clone().or_else(|| e.error.clone()))
                .unwrap_or_else(|| format!("HTTP {status}"));
            if message.to_lowercase().contains("two factor") {
                return Err(Error::TwoFactorRequired);
            }
            return Err(Error::Rejected(message));
        }

        let auth: AuthResponse = serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("unexpected Ely.by response: {e}")))?;
        let profile = auth
            .selected_profile
            .ok_or_else(|| Error::Rejected("this account has no Minecraft profile".into()))?;
        Ok(Session {
            access_token: auth.access_token,
            client_token: auth.client_token,
            uuid: profile.id.replace('-', ""),
            name: profile.name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn ok_body(token: &str) -> serde_json::Value {
        json!({
            "accessToken": token,
            "clientToken": "client",
            "selectedProfile": {"id": "ffc8fdc9-5824-509e-8a57-c99b940fb996", "name": "ErickSkrauch"}
        })
    }

    #[tokio::test]
    async fn signs_in_and_undashes_the_uuid() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/auth/authenticate"))
            .and(body_partial_json(
                json!({"username": "erick@ely.by", "password": "secret", "clientToken": "client"}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body("t1")))
            .mount(&server)
            .await;

        let ely = ElyClient::with_base(reqwest::Client::new(), server.uri());
        let session = ely
            .authenticate(" erick@ely.by ", "secret", None, "client")
            .await
            .unwrap();
        assert_eq!(session.uuid, "ffc8fdc95824509e8a57c99b940fb996");
        assert_eq!(
            (session.name.as_str(), session.access_token.as_str()),
            ("ErickSkrauch", "t1")
        );
    }

    #[tokio::test]
    async fn two_factor_and_bad_password_are_told_apart() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(json!({"password": "secret"})))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": "ForbiddenOperationException", "errorMessage": "Account protected with two factor auth."
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(body_partial_json(json!({"password": "secret:123456"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body("t2")))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": "ForbiddenOperationException", "errorMessage": "Invalid credentials. Invalid nickname or password."
            })))
            .mount(&server)
            .await;

        let ely = ElyClient::with_base(reqwest::Client::new(), server.uri());
        assert!(matches!(
            ely.authenticate("erick", "secret", None, "c").await,
            Err(Error::TwoFactorRequired)
        ));
        assert_eq!(
            ely.authenticate("erick", "secret", Some("123456"), "c")
                .await
                .unwrap()
                .access_token,
            "t2"
        );
        let err = ely
            .authenticate("erick", "wrong", None, "c")
            .await
            .unwrap_err();
        assert!(
            matches!(&err, Error::Rejected(m) if m.contains("Invalid credentials")),
            "{err}"
        );
    }

    #[tokio::test]
    async fn ensure_valid_refreshes_rejected_tokens_and_reports_expiry() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/auth/validate"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/auth/refresh"))
            .and(body_partial_json(json!({"accessToken": "old"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body("new")))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/auth/refresh"))
            .respond_with(ResponseTemplate::new(401).set_body_json(
                json!({"error": "ForbiddenOperationException", "errorMessage": "Token expired."}),
            ))
            .mount(&server)
            .await;

        let ely = ElyClient::with_base(reqwest::Client::new(), server.uri());
        let old = Session {
            access_token: "old".into(),
            client_token: "client".into(),
            uuid: "u".into(),
            name: "n".into(),
        };
        assert_eq!(
            ely.ensure_valid(old.clone()).await.unwrap().access_token,
            "new"
        );

        let dead = Session {
            access_token: "dead".into(),
            ..old
        };
        assert!(matches!(
            ely.ensure_valid(dead).await,
            Err(Error::SessionExpired)
        ));
    }
}
