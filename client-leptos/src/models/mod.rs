use serde::Serialize;

#[derive(Serialize, Clone, Debug, Default)]
pub struct LoginData {
    pub c2_addr: String,
    pub username: String,
    pub password: String,
}

pub const C2_STORAGE_KEY: &str = "WYRM_C2_ADDR";
