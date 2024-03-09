use crate::mp::UserName;

#[derive(Clone, Default)]
pub struct ConState {
    pub un: Option<UserName>,
}
