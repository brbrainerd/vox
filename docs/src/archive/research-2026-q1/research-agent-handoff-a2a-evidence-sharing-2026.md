---
title: "Cross-Agent Evidence Sharing in A2A Protocol Implementations"
category: "Architecture SSOTs"
status: "research"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---
**5\. Cross-Agent Evidence Sharing in A2A Protocol Implementations**

*Evidence Quality Rating: Medium (Based on protocol specifications, GitHub repository architecture discussions, and developer implementation patterns).*  
The "Remote relay ordering hazard" gap is fundamentally an issue of how evidence is serialized, authorized, and transported across network boundaries. The A2A protocol provides specific data models for cross-agent evidence sharing, primarily distinguishing between inline embedding and durable artifact references, each carrying distinct implications for latency, trust, and accuracy.5

### **5.1 Inline Embedding (Message Parts)**

Inline embedding packages text or structured JSON data directly within the A2A Message Part payload.5

* **Latency and Implementation:** This approach provides the lowest latency for small metadata exchanges and configuration details. It allows for immediate, synchronous parsing via JSON schema negotiation between agents.5  
* **Trust and Accuracy Implications:** Inline messages are explicitly **not** considered a reliable delivery mechanism for critical information and are not guaranteed to be persisted in the A2A Task History.5 Relying on inline embedding for large context chunks introduces severe context bloat to the receiving agent. It also violates zero-trust principles, as it forces the receiver to parse potentially un-sanitized, poisoned text directly into its active prompt, increasing the risk of cross-agent prompt injection attacks.61

### **5.2 Durable Artifact References**

For substantial evidence sharing, the A2A protocol heavily recommends the use of Artifacts containing file or URL references.5 Rather than sending a massive dataset inline, the delegating agent sends a secure URI pointing to external storage.

* **Trust and Accuracy Implications:** This is the most secure and accurate sharing mechanism, forming the backbone of Opaque Execution.5 The receiving agent can pull the data asynchronously. Crucially, the URI incorporates temporary authentication credentials (e.g., short-lived OAuth tokens). This adheres to On-Behalf-Of (OBO) token flows, ensuring that the receiving agent inherits the original user's identity authorization and scope, preventing privilege escalation or unauthorized data access.35  
* **Latency Implications:** While it introduces a secondary network hop (the receiving agent must re-retrieve the data from the URI), it protects the system from distributed context bloat. The receiving agent can choose to map the artifact into its own local vector space, apply a selective "Socrates gate" extraction, or stream "artifact chunks" in real-time as they are generated, drastically optimizing the total token processing latency of the overarching workflow.5

## ---

*(Original Source: AI Agent Context and Handoff Research)*

