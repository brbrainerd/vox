//! # vox-toestub
//!
//! **T**odo, **O**mitted wiring, **E**mpty bodies, **S**tub functions,
//! **T**oo-early victory, **U**nresolved references, **B**roken DRY — detector.
//!
//! TOESTUB mechanically detects AI coding anti-patterns that are banned by
//! AGENTS.md but otherwise only caught during manual review.

pub mod ai_analyze;
pub mod detectors;
pub mod engine;
pub mod report;
pub mod review;
pub mod rules;
pub mod scanner;
pub mod task_queue;

pub use ai_analyze::{AiAnalyzer, AiProvider};
pub use engine::{ToestubConfig, ToestubEngine};
pub use report::{OutputFormat, Reporter};
pub use review::{
    auto_discover_providers, build_diff_review_prompt, build_review_prompt, format_markdown,
    format_sarif, format_terminal, parse_review_response, ReviewCategory, ReviewClient,
    ReviewConfig, ReviewFinding, ReviewOutputFormat, ReviewProvider, ReviewResult,
};
pub use rules::{DetectionRule, Finding, Language, Severity};
pub use scanner::Scanner;
pub use task_queue::TaskQueue;
