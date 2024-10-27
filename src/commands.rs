
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Noop,
    Connect(String),
    Pub{subject: String, msg: String},
    Sub{subject: String, id: String},
    Ping,
    Pong,
}