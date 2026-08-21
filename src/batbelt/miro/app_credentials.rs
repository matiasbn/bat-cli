//! Credentials of the shared Miro app that bat-cli authorizes against.
//!
//! # Why this file exists
//!
//! OAuth needs a `client_id` and a `client_secret` to exchange the authorization
//! code, and Miro does not document PKCE — the mechanism that lets a public CLI
//! avoid shipping a secret. There is also no API to discover a user's apps, so
//! bat-cli cannot learn these values on its own.
//!
//! Filling them in here means **nobody else has to create a Miro app**. One app
//! serves every user: each of them runs `bat-cli login`, lands on Miro's consent
//! page, picks their own team and presses Accept. That is what OAuth is for.
//!
//! # Why not an environment variable at build time
//!
//! `option_env!` is evaluated when the crate is compiled, and `cargo install
//! bat-cli` compiles on the *user's* machine, where those variables are not set.
//! Env vars therefore only work for binaries you build and hand out yourself.
//! Constants in the source travel with the published crate, which is what makes
//! the zero-paste flow work for everyone.
//!
//! # What this exposes
//!
//! A client secret inside a distributed binary is extractable — treat it as
//! public. It does **not** grant access to any board: every user still has to
//! authorize explicitly, and the redirect goes to `localhost`. The realistic
//! risk is somebody running a consent screen under your app's name.
//!
//! Leave both empty to keep the per-user flow, where `bat-cli login --setup`
//! asks each user for their own app.

/// Client ID of the shared app. Empty means "no shared app configured".
pub const CLIENT_ID: &str = "";

/// Client secret of the shared app.
pub const CLIENT_SECRET: &str = "";

/// The shared app, if one was configured.
pub fn shared_app() -> Option<(&'static str, &'static str)> {
    if CLIENT_ID.is_empty() || CLIENT_SECRET.is_empty() {
        return None;
    }
    Some((CLIENT_ID, CLIENT_SECRET))
}

#[cfg(test)]
mod app_credentials_test {
    use super::*;

    /// Both constants must be set together: a client id without a secret would
    /// send the user to a consent page whose code cannot be exchanged.
    #[test]
    fn test_shared_app_is_all_or_nothing() {
        assert_eq!(
            CLIENT_ID.is_empty(),
            CLIENT_SECRET.is_empty(),
            "set both CLIENT_ID and CLIENT_SECRET, or neither"
        );
        assert_eq!(shared_app().is_some(), !CLIENT_ID.is_empty());
    }
}
