use parking_lot::Mutex;

pub enum Token {
    AccessToken,
    RefreshToken,
    TokenDuration,
    StateToken,
}

pub struct AuthState {
    pub access_token: Mutex<String>,
    pub refresh_token: Mutex<String>,
    pub token_duration: Mutex<String>,
    pub state_state: Mutex<String>,
}

impl AuthState {
    pub fn retrieve(&self, t: Token) -> String {
        return match t {
            Token::AccessToken => self.access_token.lock(),
            Token::RefreshToken => self.refresh_token.lock(),
            Token::TokenDuration => self.token_duration.lock(),
            Token::StateToken => self.state_state.lock(),
        }
        .to_string();
    }

    pub fn write(&self, t: Token, s: String) {
        *match t {
            Token::AccessToken => self.access_token.lock(),
            Token::RefreshToken => self.refresh_token.lock(),
            Token::TokenDuration => self.token_duration.lock(),
            Token::StateToken => self.state_state.lock(),
        } = s;
    }
}
