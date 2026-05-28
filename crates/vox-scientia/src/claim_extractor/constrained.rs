use serde_json::{Value, json};

pub fn claim_envelope_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "required": ["id", "text", "verifiability", "verifiability_score"],
        "additionalProperties": false,
        "properties": {
            "id": { "type": "integer", "minimum": 0 },
            "text": { "type": "string", "minLength": 5, "maxLength": 500 },
            "verifiability": { "type": "string", "enum": ["numeric", "structured", "semantic", "event_based", "unverifiable"] },
            "verifiability_score": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
            "tuple": { "type": ["object", "null"] }
        }
    })
}

pub fn validate_claim_envelope(claim_json: &Value) -> Result<(), String> {
    let required = ["id", "text", "verifiability", "verifiability_score"];
    for field in &required {
        if claim_json.get(field).is_none() {
            return Err(format!("missing required field: {field}"));
        }
    }
    if let Some(score) = claim_json["verifiability_score"].as_f64()
        && !(0.0..=1.0).contains(&score)
    {
        return Err("verifiability_score out of range".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_envelope_passes() {
        let v = json!({
            "id": 12345,
            "text": "Latency rose by 10ms",
            "verifiability": "numeric",
            "verifiability_score": 0.85
        });
        assert!(validate_claim_envelope(&v).is_ok());
    }

    #[test]
    fn missing_field_fails() {
        let v = json!({"id": 1, "text": "x"});
        assert!(validate_claim_envelope(&v).is_err());
    }
}
