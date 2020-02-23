use crate::lantern_db;
use crate::user_db;

pub struct GlobalState {
    pub lantern_db_addr: actix::prelude::Addr<lantern_db::LanternDb>,
    pub user_db_addr: actix::prelude::Addr<user_db::UserDb>,
    pub password_hash: String,
    pub password_salt: String,
    pub root_path: String,
}
