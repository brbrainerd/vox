---
title: "MENS Quality Scoring System"
description: "Quality scoring system for MENS training data — per-pair ratings, AST diversity, and domain thresholds."
category: "Language Reference"
status: "current"
---
# MENS Quality Scoring System

## Overview

The MENS pipeline includes a comprehensive quality scoring system to ensure training data meets diversity and quality thresholds before triggering training runs. This system operates at multiple levels: per-pair quality ratings, AST diversity analysis, and domain-specific thresholds.

## Components

### 1. Per-Pair Quality Ratings

Each training pair in the corpus carries a `rating` field (0-5 scale) that indicates quality:

- **Rating 5**: Golden examples, hand-verified, canonical patterns
- **Rating 4**: High-quality extracted code, validated by compiler
- **Rating 3**: Standard quality, passes basic validation
- **Rating 2-0**: Lower quality, typically filtered out or heavily downsampled

**Implementation:**
- Corpus extract modules (`extract_vox`, `extract_rs`, `extract_docs`) assign `default_rating` to extracted pairs
- Golden examples inherit ratings from their frontmatter metadata
- Domain profiles set `min_rating` thresholds (e.g., vox-lang requires rating ≥4)

**Configuration:**
```yaml
# mens/config/domain-profiles.yaml
defaults:
  min_rating: 3

profiles:
  vox-lang:
    min_rating: 4  # Higher threshold for core language adapter
```

### 2. AST Diversity Matrix

The flywheel system uses AST hashing to measure semantic diversity of new data before triggering training.

**Algorithm:**
1. Parse each training pair's Vox code into an AST
2. Compute a hash of the AST structure (ignoring literals, identifiers)
3. Track unique AST signatures across the dataset
4. Calculate diversity score: `unique_signatures / total_samples`

**Purpose:**
- Prevents training on monoculture data (e.g., 1000 identical "hello world" mutations)
- Ensures new data introduces novel patterns
- Domain-specific thresholds (agents require higher diversity than rust-expert)

**Configuration:**
```yaml
# mens/config/flywheel.yaml
min_ast_diversity: 0.40  # 40% of samples must have unique AST structures

domains:
  rust-expert:
    min_ast_diversity: 0.35  # Rust patterns more repetitive, lower threshold
  agents:
    min_ast_diversity: 0.50  # Agent trajectories need high diversity
```

**Implementation:**
- `crates/vox-corpus/src/flywheel.rs` - AST diversity calculation
- Uses semantic diversity matrix (Wave 3-03)
- Computes diversity per domain before training trigger

### 3. Synthetic Data Diversity Floors

The synthetic corpus generator enforces minimum diversity for generated content:

**Configuration:**
```rust
pub struct SyntheticGenConfig {
    pub min_phrasings_per_tool: usize,  // Minimum phrasings per tool call
    pub min_pairs_per_a2a_type: usize,  // Minimum A2A pairs per message type
}
```

**Purpose:**
- Ensures synthetic data covers tool use patterns comprehensively
- Prevents over-representation of common phrasings
- Guarantees coverage of A2A message types

### 4. Domain-Specific Quality Thresholds

Different domains have different quality requirements based on their characteristics:

| Domain | min_rating | min_ast_diversity | Rationale |
|--------|-----------|-------------------|-----------|
| vox-lang | 4 | 0.40 | Core language requires high-quality canonical patterns |
| rust-expert | 3 | 0.35 | Rust patterns more repetitive, lower diversity acceptable |
| agents | 4 | 0.50 | Agent trajectories need high diversity for robustness |
| research | 4 | 0.40 | Research synthesis requires quality and novelty |
| rocks | 4 | 0.40 | Database patterns need correctness |

## Quality Gates

### Pre-Training Gates

Before training begins, the pipeline enforces:

1. **Sample Floor**: Minimum number of new samples (default: 500)
2. **AST Diversity Gate**: Diversity score must exceed threshold
3. **Rating Gate**: Pairs below `min_rating` are filtered out

```rust
// crates/vox-corpus/src/flywheel.rs
pub fn check(&self, current_samples: usize, current_diversity: f64) -> FlywheelSignal {
    if current_samples < self.config.sample_floor {
        return FlywheelSignal::Pending { new_samples: current_samples };
    }
    if current_diversity < self.config.min_ast_diversity {
        return FlywheelSignal::Idle; // Diversity gate failed
    }
    FlywheelSignal::Ready { ast_diversity: current_diversity }
}
```

### Post-Training Evaluation Gates

After training, evaluation gates enforce:

1. **Parse Rate**: Minimum fraction of generated code that parses as valid Vox
2. **Coverage**: Minimum coverage of language constructs
3. **Anti-Stub**: Maximum rate of placeholder/generic responses

```yaml
# mens/config/eval-gates-post-train.yaml
eval_local:
  min_parse_rate: 0.25  # Bootstrap gate
  min_coverage: 0.10    # Minimum construct diversity
```

## Data Source Quality Tracking

The corpus mix report includes per-source quality metrics:

- **Input Lines**: Total lines read from source
- **Emitted Lines**: Lines that passed filters
- **Skipped Reason**: Why lines were skipped (missing_file, weight_zero, no_lines_passed_filters)
- **Share of Output**: Fraction of total output contributed by this source

**Report Location:**
- `{output_path}.mix_report.json` (e.g., `target/dogfood/train_mixed.mix_report.json`)

## Monitoring and Telemetry

Quality metrics are tracked via:

1. **Mix Reports**: JSON reports after each corpus mix
2. **Flywheel Signals**: Pending/Ready/Idle/Triggered states
3. **Evaluation Reports**: Post-training quality metrics

## Best Practices

1. **Maintain High Rating Thresholds**: Core domains (vox-lang, agents) should use rating ≥4
2. **Monitor Diversity Scores**: If diversity drops below threshold, investigate data quality
3. **Review Mix Reports**: Check which sources contribute most/least to output
4. **Domain-Specific Tuning**: Adjust thresholds based on domain characteristics
5. **Synthetic Data Quality**: Use diversity floors to prevent monoculture in synthetic generation

## References

- Configuration: `mens/config/flywheel.yaml`
- Implementation: `crates/vox-corpus/src/flywheel.rs`
- Domain Profiles: `mens/config/domain-profiles.yaml`
- Mix Configuration: `mens/config/mix.yaml`
- Evaluation Gates: `mens/config/eval-gates-post-train.yaml`
