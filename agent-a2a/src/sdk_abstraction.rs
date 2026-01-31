use crate::skills::Skill;

pub trait RadkitAdapter: Send + Sync {
    fn version(&self) -> &str;
    fn create_runtime(&self) -> Result<Box<dyn RadkitRuntime>, String>;
    fn register_skill(
        &self,
        runtime: &mut dyn RadkitRuntime,
        skill: Box<dyn Skill>
    ) -> Result<(), String>;
    fn start_server(
        &self,
        runtime: Box<dyn RadkitRuntime>,
        addr: std::net::SocketAddr
    ) -> Result<(), String>;
}

pub trait RadkitRuntime: Send + Sync {
    fn add_skill(&mut self, skill: Box<dyn Skill>);
}

pub struct RadkitV0Adapter;

impl RadkitV0Adapter {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for RadkitV0Adapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RadkitAdapter for RadkitV0Adapter {
    fn version(&self) -> &str {
        "0.0.4"
    }

    fn create_runtime(&self) -> Result<Box<dyn RadkitRuntime>, String> {
        Ok(Box::new(RadkitV0Runtime::new()))
    }

    fn register_skill(
        &self,
        runtime: &mut dyn RadkitRuntime,
        skill: Box<dyn Skill>
    ) -> Result<(), String> {
        runtime.add_skill(skill);
        Ok(())
    }

    fn start_server(
        &self,
        _runtime: Box<dyn RadkitRuntime>,
        _addr: std::net::SocketAddr
    ) -> Result<(), String> {
        Ok(())
    }
}

pub struct RadkitV0Runtime {
    skills: Vec<Box<dyn Skill>>
}

impl RadkitV0Runtime {
    #[must_use]
    pub fn new() -> Self {
        Self { skills: Vec::new() }
    }
}

impl Default for RadkitV0Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl RadkitRuntime for RadkitV0Runtime {
    fn add_skill(&mut self, skill: Box<dyn Skill>) {
        self.skills.push(skill);
    }
}
