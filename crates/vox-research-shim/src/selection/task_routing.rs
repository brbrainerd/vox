use vox_orchestrator::mode::InferenceConfig;
use vox_orchestrator::models::ModelSpec;
use vox_orchestrator::types::{RoutingProfile, TaskCategory};

/// Map task category and capability flags to a routing profile for telemetry and specialist routing.
pub fn task_and_flags_to_profile(
    task: TaskCategory,
    requires_vision: bool,
    requires_web_search: bool,
    requires_structured_output: bool,
) -> RoutingProfile {
    if requires_vision {
        return RoutingProfile::Vision;
    }
    if requires_web_search || task == TaskCategory::Research {
        return RoutingProfile::Research;
    }
    if requires_structured_output {
        return RoutingProfile::StrictJson;
    }
    match task {
        TaskCategory::Review => RoutingProfile::VoxComposer,
        TaskCategory::Planning => RoutingProfile::Planning,
        TaskCategory::CodeGen | TaskCategory::Testing => RoutingProfile::General,
        TaskCategory::Debugging | TaskCategory::TypeChecking | TaskCategory::Parsing => {
            RoutingProfile::RustLangdev
        }
        TaskCategory::Research | TaskCategory::Ars => RoutingProfile::Research,
        // All other categories (General, InterAgent, ToolOrchestration, Visus, …)
        _ => RoutingProfile::General,
    }
}

/// Canonical mapping: TaskCategory → strength tags used for filtering and scoring.
/// Add new TaskCategory or strength only here.
pub fn task_strengths(task_type: TaskCategory) -> &'static [&'static str] {
    match task_type {
        TaskCategory::CodeGen => &["codegen"],
        TaskCategory::Testing => &["codegen"],
        TaskCategory::Debugging => &["debugging", "logic", "reasoning"],
        TaskCategory::TypeChecking => &["logic", "parsing"],
        TaskCategory::Research => &["research", "codegen"],
        TaskCategory::Parsing => &["parsing", "codegen"],
        TaskCategory::Review => &["review", "codegen"],
        TaskCategory::Planning => &["logic", "reasoning", "research"],
        TaskCategory::Ars => &["logic", "reasoning", "codegen"],
        TaskCategory::InterAgent | TaskCategory::ToolOrchestration | TaskCategory::Visus => {
            &["logic", "reasoning", "codegen"]
        }
        // General and any future variants: broad generalist signal.
        _ => &["generalist"],
    }
}

/// Primary strength for a task (used when a single tag is needed).
pub fn primary_strength(task_type: TaskCategory) -> &'static str {
    task_strengths(task_type)[0]
}

/// Returns true if the model has any strength matching the task.
pub fn model_matches_task(model: &ModelSpec, task_type: TaskCategory) -> bool {
    let strengths = task_strengths(task_type);
    model
        .strengths
        .iter()
        .any(|s| strengths.contains(&s.to_string().as_str()))
}

/// Resolve a `RoutingProfile` from an `InferenceConfig` and task category.
///
/// This is the `InferenceConfig`-native replacement for constructing the profile
/// from individual `requires_vision`, `requires_web_search`, etc. booleans.
pub fn config_to_routing_profile(task: TaskCategory, cfg: &InferenceConfig) -> RoutingProfile {
    task_and_flags_to_profile(
        task,
        cfg.modalities.vision,
        cfg.modalities.web_search,
        cfg.modalities.structured_output,
    )
}
