

/// Namespace management for the content-addressed store.
pub struct Namespace {
    segments: Vec<String>,
}

impl Namespace {
    pub fn root() -> Self {
        Self { segments: vec![] }
    }

    pub fn new(path: &str) -> Self {
        Self {
            segments: path.split('.').map(|s| s.to_string()).collect(),
        }
    }

    pub fn child(&self, name: &str) -> Self {
        let mut segments = self.segments.clone();
        segments.push(name.to_string());
        Self { segments }
    }

    pub fn to_string(&self) -> String {
        if self.segments.is_empty() {
            ".".to_string()
        } else {
            self.segments.join(".")
        }
    }

    pub fn parent(&self) -> Option<Self> {
        if self.segments.is_empty() {
            None
        } else {
            let mut segments = self.segments.clone();
            segments.pop();
            Some(Self { segments })
        }
    }
}
