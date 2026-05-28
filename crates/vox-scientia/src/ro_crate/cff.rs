use serde_json::{Value, json};

pub struct CffAuthor {
    pub given_names: String,
    pub family_names: String,
    pub orcid: Option<String>,
}

pub struct CffMetadata {
    pub title: String,
    pub version: String,
    pub doi: Option<String>,
    pub authors: Vec<CffAuthor>,
    pub repository_url: String,
    pub license: String,
}

pub fn build_cff_json(cff: &CffMetadata) -> Value {
    let authors: Vec<Value> = cff
        .authors
        .iter()
        .map(|a| {
            let mut author = json!({
                "given-names": a.given_names,
                "family-names": a.family_names,
            });
            if let Some(orcid) = &a.orcid {
                author["orcid"] = json!(orcid);
            }
            author
        })
        .collect();

    let mut result = json!({
        "cff-version": "1.2.0",
        "message": "If you use this software, please cite it as below.",
        "title": cff.title,
        "version": cff.version,
        "license": cff.license,
        "repository-code": cff.repository_url,
        "authors": authors
    });

    if let Some(doi) = &cff.doi {
        result["doi"] = json!(doi);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_cff() -> CffMetadata {
        CffMetadata {
            title: "vox-scientia".to_string(),
            version: "0.1.0".to_string(),
            doi: None,
            authors: vec![CffAuthor {
                given_names: "Jane".to_string(),
                family_names: "Doe".to_string(),
                orcid: None,
            }],
            repository_url: "https://github.com/example/vox".to_string(),
            license: "MIT".to_string(),
        }
    }

    #[test]
    fn cff_has_required_fields() {
        let json = build_cff_json(&sample_cff());
        assert_eq!(json["cff-version"], "1.2.0");
        assert!(json["message"].as_str().is_some());
        assert_eq!(json["title"], "vox-scientia");
    }

    #[test]
    fn author_orcid_included_when_present() {
        let mut cff = sample_cff();
        cff.authors[0].orcid = Some("https://orcid.org/0000-0001-2345-6789".to_string());
        let json = build_cff_json(&cff);
        let authors = json["authors"].as_array().unwrap();
        assert_eq!(authors[0]["orcid"], "https://orcid.org/0000-0001-2345-6789");
    }
}
