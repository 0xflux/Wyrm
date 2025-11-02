use serde::Serialize;

#[derive(Serialize, Clone, Debug, Default)]
pub struct LoginData {
    pub c2_addr: String,
    pub username: String,
    pub password: String,
    pub admin_env_token: String,
}
