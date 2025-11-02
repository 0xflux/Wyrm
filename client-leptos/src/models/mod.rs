use reactive_stores::Store;
use serde::Serialize;

#[derive(Clone, Debug, Default, Store)]
pub struct GlobalState {
    credentials: Option<LoginData>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct LoginData {
    pub c2_addr: String,
    pub username: String,
    pub password: String,
    pub admin_env_token: String,
}
