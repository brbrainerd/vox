//! Fluent builders that produce valid Plugin.toml TOML strings for tests.

/// Builds a minimal Code-kind Plugin.toml TOML string.
pub struct CodeManifestBuilder {
    id: String,
    name: String,
    version: String,
    description: String,
    min_vox_version: String,
    abi_version: u32,
    extension_points: Vec<String>,
    artifacts: Vec<(String, String)>,
    status: Option<String>,
    category: Option<String>,
    tags: Vec<String>,
}

impl CodeManifestBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            version: "0.1.0".into(),
            description: "Test plugin.".into(),
            min_vox_version: "0.5.0".into(),
            abi_version: 1,
            extension_points: Vec::new(),
            artifacts: Vec::new(),
            status: None,
            category: None,
            tags: Vec::new(),
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
    pub fn version(mut self, v: impl Into<String>) -> Self {
        self.version = v.into();
        self
    }
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = d.into();
        self
    }
    pub fn min_vox_version(mut self, v: impl Into<String>) -> Self {
        self.min_vox_version = v.into();
        self
    }
    pub fn abi_version(mut self, v: u32) -> Self {
        self.abi_version = v;
        self
    }
    pub fn extension_point(mut self, ep: impl Into<String>) -> Self {
        self.extension_points.push(ep.into());
        self
    }
    pub fn artifact(mut self, target: impl Into<String>, file: impl Into<String>) -> Self {
        self.artifacts.push((target.into(), file.into()));
        self
    }
    pub fn status(mut self, s: impl Into<String>) -> Self {
        self.status = Some(s.into());
        self
    }
    pub fn category(mut self, c: impl Into<String>) -> Self {
        self.category = Some(c.into());
        self
    }
    pub fn tag(mut self, t: impl Into<String>) -> Self {
        self.tags.push(t.into());
        self
    }

    /// Render to a Plugin.toml TOML string.
    pub fn build(self) -> String {
        let mut s = format!(
            "[plugin]\n\
             id = {:?}\n\
             name = {:?}\n\
             version = {:?}\n\
             description = {:?}\n",
            self.id, self.name, self.version, self.description
        );
        if let Some(st) = &self.status {
            s.push_str(&format!("status = {:?}\n", st));
        }
        if let Some(cat) = &self.category {
            s.push_str(&format!("category = {:?}\n", cat));
        }
        if !self.tags.is_empty() {
            let tags_toml: Vec<String> = self.tags.iter().map(|t| format!("{:?}", t)).collect();
            s.push_str(&format!("tags = [{}]\n", tags_toml.join(", ")));
        }
        s.push_str(&format!(
            "\n[plugin.host]\nmin-vox-version = {:?}\n\
             \n[plugin.payload]\nkind = \"code\"\nabi-version = {}\n",
            self.min_vox_version, self.abi_version
        ));
        if !self.extension_points.is_empty() {
            let eps: Vec<String> = self
                .extension_points
                .iter()
                .map(|e| format!("{:?}", e))
                .collect();
            s.push_str(&format!(
                "\n[plugin.payload.provides]\nextension-points = [{}]\n",
                eps.join(", ")
            ));
        }
        if !self.artifacts.is_empty() {
            s.push_str("\n[plugin.payload.artifacts]\n");
            for (target, file) in &self.artifacts {
                s.push_str(&format!("{:?} = {:?}\n", target, file));
            }
        }
        s
    }
}

/// Builds a minimal Skill-kind Plugin.toml TOML string.
pub struct SkillManifestBuilder {
    id: String,
    name: String,
    version: String,
    description: String,
    min_vox_version: String,
    format_version: u32,
    skill_md: String,
    exposes: Vec<String>,
}

impl SkillManifestBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            version: "0.1.0".into(),
            description: "Test skill plugin.".into(),
            min_vox_version: "0.5.0".into(),
            format_version: 1,
            skill_md: "SKILL.md".into(),
            exposes: Vec::new(),
        }
    }

    pub fn name(mut self, n: impl Into<String>) -> Self {
        self.name = n.into();
        self
    }
    pub fn version(mut self, v: impl Into<String>) -> Self {
        self.version = v.into();
        self
    }
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = d.into();
        self
    }
    pub fn skill_md(mut self, path: impl Into<String>) -> Self {
        self.skill_md = path.into();
        self
    }
    pub fn exposes(mut self, tool: impl Into<String>) -> Self {
        self.exposes.push(tool.into());
        self
    }

    pub fn build(self) -> String {
        let mut s = format!(
            "[plugin]\n\
             id = {:?}\n\
             name = {:?}\n\
             version = {:?}\n\
             description = {:?}\n\
             \n[plugin.host]\nmin-vox-version = {:?}\n\
             \n[plugin.payload]\nkind = \"skill\"\nformat-version = {}\nskill-md = {:?}\n",
            self.id,
            self.name,
            self.version,
            self.description,
            self.min_vox_version,
            self.format_version,
            self.skill_md
        );
        if !self.exposes.is_empty() {
            let tools: Vec<String> = self.exposes.iter().map(|t| format!("{:?}", t)).collect();
            s.push_str(&format!(
                "\n[plugin.payload.tools]\nexposes = [{}]\n",
                tools.join(", ")
            ));
        }
        s
    }
}
