# Example: sharing.vox

```vox
# sharing.vox
# Example demonstrating workflow sharing, skill definition, and code snippet sharing

@skill fn DataSummarizer(text: str) to str:
    # A reusable AI capabilities skill that can be published to the Marketplace
    "Summary of " + text

workflow process_document(doc_id: str) to Result[bool]:
    let doc = db.documents.find(doc_id)
    let summary = DataSummarizer(doc.content)
    db.documents.update(doc_id, summary)
    ret Ok(true)

# Publishable UI Component
@component fn SharedChart(data: list[int]) to Element:
    <div class="chart-box">
        {data.len()} data points rendered
    </div>

style:
    .chart-box:
        padding: "16px"
        background: "#fafafa"
        border: "1px solid #ddd"
```
