use russh::server::Auth;

pub struct AuthHandler;

impl AuthHandler {
    pub fn auth_none() -> Auth {
        Auth::Accept
    }

    pub fn auth_password(_username: &str, _password: &str) -> Auth {
        Auth::Accept
    }
}
