use crate::core::Core;

pub struct Server {
    core: Core
}

impl Server {
    pub fn new() -> Self {
        Self {
            core: Core::new()
        }
    }
}
