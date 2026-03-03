use clap::Parser;

#[derive(Parser)]
pub enum ArchitectAction {
    /// Validate workspace architecture against vox-schema.json
    Check,
    /// Automatically move crates to their schema-correct locations
    FixSprawl {
        /// Actually move files (default: false, dry-run only)
        #[arg(long)]
        apply: bool,
    },
    /// Analyze God Objects and suggest trait decomposition
    Analyze {
        /// Path to source file or directory
        #[arg(default_value = ".")]
        path: std::path::PathBuf,
    },
}

#[derive(Parser)]
pub enum ShareAction {
    /// Publish an artifact to the marketplace
    Publish {
        /// Type: workflow, skill, snippet, agent
        #[arg(long)]
        r#type: String,
        /// Artifact name
        #[arg(long)]
        name: String,
        /// Content hash (from vox-pm store)
        #[arg(long)]
        hash: String,
        /// Version string
        #[arg(long, default_value = "0.1.0")]
        version: String,
        /// Description
        #[arg(long)]
        description: Option<String>,
        /// Comma-separated tags
        #[arg(long)]
        tags: Option<String>,
    },
    /// Search the marketplace
    Search {
        /// Query string
        query: String,
    },
    /// List artifacts by type
    List {
        /// Type: workflow, skill, snippet, agent
        r#type: String,
    },
    /// Review an artifact
    Review {
        /// Artifact ID
        artifact_id: String,
        /// Rating (1-5)
        #[arg(long)]
        rating: i64,
        /// Comment
        #[arg(long)]
        comment: Option<String>,
    },
}

#[derive(Parser)]
pub enum SnippetAction {
    /// Save a code snippet from a file
    Save {
        /// Path to the source file
        file: std::path::PathBuf,
        /// Title for the snippet
        #[arg(long)]
        title: String,
        /// Description
        #[arg(long)]
        description: Option<String>,
        /// Comma-separated tags
        #[arg(long)]
        tags: Option<String>,
    },
    /// Search saved snippets
    Search {
        /// Query string
        query: String,
    },
    /// Export snippets as JSON
    Export {
        /// Max number to export
        #[arg(long, default_value = "100")]
        limit: i64,
    },
}

#[derive(Parser)]
pub enum AgentAction {
    /// Register a new agent definition
    Create {
        /// Agent name
        #[arg(long)]
        name: String,
        /// Description
        #[arg(long)]
        description: Option<String>,
        /// System prompt
        #[arg(long)]
        system_prompt: Option<String>,
        /// Tools (JSON array)
        #[arg(long)]
        tools: Option<String>,
        /// Model config (JSON)
        #[arg(long)]
        model_config: Option<String>,
        /// Make the agent public
        #[arg(long)]
        public: bool,
    },
    /// List all registered agents
    List,
    /// Show details of a specific agent
    Info {
        /// Agent ID
        id: String,
    },
    /// Dynamically generate Vox agents based on workspace crates
    Generate,
}

#[derive(Parser)]
pub enum DbAction {
    /// Show current VoxDB schema version and data directory
    Status,
    /// Apply any pending schema migrations
    Migrate {
        /// Optional path to the .vox source file containing the schema
        #[arg(short, long)]
        file: Option<std::path::PathBuf>,
    },
    /// Reset the database by dropping all user tables and re-applying migrations
    Reset {
        /// Optional path to the .vox source file containing the schema
        #[arg(short, long)]
        file: Option<std::path::PathBuf>,
    },
    /// Print the current schema digest for LLM context
    Schema {
        /// Optional path to the .vox source file containing the schema
        #[arg(short, long)]
        file: Option<std::path::PathBuf>,
    },
    /// Print sample data from a table or collection
    Sample {
        /// Table or collection name
        table_name: String,
        /// Max number of rows to show
        #[arg(short, long, default_value = "10")]
        limit: i64,
    },
    /// Export state/memory for a user to JSON
    Export {
        /// The user ID to export (default: 'default')
        #[arg(long, default_value = "default")]
        user_id: String,
        /// Output file path (if omitted, writes to stdout)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },
    /// Import state/memory from a JSON file
    Import {
        /// Input file path
        path: std::path::PathBuf,
    },
    /// Reclaim space and defragment the database
    Vacuum,
    /// Prune old memory entries
    Prune {
        /// The user ID to prune for
        #[arg(long, default_value = "default")]
        user: String,
        /// Days to keep
        #[arg(long, default_value = "30")]
        days: u32,
    },
    /// Get a user preference
    PrefGet {
        /// The user ID
        #[arg(long, default_value = "default")]
        user: String,
        /// Key
        key: String,
    },
    /// Set a user preference
    PrefSet {
        /// The user ID
        #[arg(long, default_value = "default")]
        user: String,
        /// Key
        key: String,
        /// Value
        value: String,
    },
    /// List user preferences
    PrefList {
        /// The user ID
        #[arg(long, default_value = "default")]
        user: String,
        /// Filter by prefix
        #[arg(long)]
        prefix: Option<String>,
    },
}

#[derive(Parser)]
pub enum OrchestratorAction {
    /// Show all agents, queues, and file assignments
    Status,
    /// Launch the live gamified terminal HUD
    Hud,
    /// Submit a task to the orchestrator
    Submit {
        /// Task description
        #[arg(required = true)]
        description: String,
        /// Files to include (repeatable)
        #[arg(long)]
        files: Vec<String>,
        /// Priority: urgent, normal, background
        #[arg(long)]
        priority: Option<String>,
    },
    /// Show a specific agent's task queue
    Queue {
        /// Agent ID number
        #[arg(required = true)]
        agent_id: u64,
    },
    /// Trigger manual rebalancing of tasks across agents
    Rebalance,
    /// Show current orchestrator configuration
    Config,
    /// Pause an agent's queue
    Pause {
        /// Agent ID number
        #[arg(required = true)]
        agent_id: u64,
    },
    /// Resume a paused agent's queue
    Resume {
        /// Agent ID number
        #[arg(required = true)]
        agent_id: u64,
    },
    /// Save orchestrator state to disk
    Save,
    /// Load orchestrator state from disk
    Load,
}

#[derive(Parser)]
pub enum WorkflowAction {
    /// List all workflow and activity definitions in a .vox file
    List {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
    },
    /// Show detailed info about a specific workflow
    Inspect {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
        /// Workflow name to inspect
        #[arg(required = true)]
        name: String,
    },
    /// Type-check a workflow file
    Check {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
    },
    /// Run a workflow (stub for future durable execution runtime)
    Run {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
        /// Workflow name to run
        #[arg(required = true)]
        name: String,
    },
}
