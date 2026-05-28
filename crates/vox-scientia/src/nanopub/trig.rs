use sha2::{Digest, Sha256};

pub struct NanopubGraphs {
    pub assertion_graph: String,  // Turtle triples for the claim
    pub provenance_graph: String, // who generated it, when
    pub pubinfo_graph: String,    // signature embedded here
}

pub struct NanopubDocument {
    pub trig: String,   // complete TriG serialization
    pub np_uri: String, // nanopub URI (e.g. "https://vox.scientia/np/RA<hash>")
}

pub fn build_nanopub(claim_text: &str, provider_id: &str, published_at: i64) -> NanopubDocument {
    let prefixes = "@prefix : <https://vox.scientia/np/> .\n\
         @prefix np: <http://www.nanopub.org/nschema#> .\n\
         @prefix prov: <http://www.w3.org/ns/prov#> .\n\
         @prefix dc: <http://purl.org/dc/terms/> .\n\
         @prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n\
         @prefix scientia: <https://vox.scientia/vocab#> .\n\n"
        .to_string();

    let head = ":head {\n  \
           :np np:hasAssertion :assertion ;\n      \
               np:hasProvenance :provenance ;\n      \
               np:hasPublicationInfo :pubinfo .\n\
         }\n\n"
        .to_string();

    let assertion = format!(
        ":assertion {{\n  \
           :claim1 rdf:type scientia:AtomicClaim ;\n          \
                   scientia:text {claim_text:?} .\n\
         }}\n\n",
        claim_text = claim_text
    );

    let provenance = format!(
        ":provenance {{\n  \
           :assertion prov:wasAttributedTo {provider_id:?} ;\n              \
                      prov:generatedAtTime {published_at} .\n\
         }}\n\n",
        provider_id = provider_id,
        published_at = published_at
    );

    let pubinfo = format!(
        ":pubinfo {{\n  \
           :np dc:created {published_at} ;\n      \
               dc:creator \"vox-scientia\" .\n\
         }}\n",
        published_at = published_at
    );

    let trig = format!("{}{}{}{}{}", prefixes, head, assertion, provenance, pubinfo);

    let hash = hex::encode(Sha256::digest(trig.as_bytes()));
    let np_uri = format!("https://vox.scientia/np/RA{}", hash);

    NanopubDocument { trig, np_uri }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trig_document_contains_four_graphs() {
        let doc = build_nanopub("test claim", "provider:test", 1000000);
        assert!(doc.trig.contains("@prefix"));
        assert!(doc.trig.contains(":head"));
        assert!(doc.trig.contains(":assertion"));
        assert!(doc.trig.contains(":provenance"));
        assert!(doc.trig.contains(":pubinfo"));
    }

    #[test]
    fn np_uri_starts_with_prefix() {
        let doc = build_nanopub("test claim", "provider:test", 1000000);
        assert!(doc.np_uri.starts_with("https://vox.scientia/np/RA"));
    }

    #[test]
    fn same_inputs_produce_same_trig() {
        let doc1 = build_nanopub("claim", "provider:x", 42);
        let doc2 = build_nanopub("claim", "provider:x", 42);
        assert_eq!(doc1.trig, doc2.trig);
        assert_eq!(doc1.np_uri, doc2.np_uri);
    }
}
