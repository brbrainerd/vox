---
title: "Deep Research Competitive Landscape & User Sentiment (2026-08-01)"
description: "Product-by-product research on Google Deep Research, Claude Research, OpenAI Deep Research, Perplexity Deep Research, Elicit, You.com/Grok/Manus/Genspark: capabilities, disclosed technical details, and real user sentiment (praise and complaints), with synthesis on where deep research is most/least useful and the open differentiator opportunity for Vox."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Normative competitive research grounding the deep-research enhancement program's differentiation strategy; identifies verifiable-trust-infrastructure as the open competitive gap as of 2026-08-01."
---

# Deep Research Competitive Landscape & User Sentiment

**Date:** 2026-08-01
**Scope:** Google Deep Research, Anthropic Claude Research, OpenAI Deep Research, Perplexity Deep Research, Elicit, and other notable agentic-research tools (You.com/ARI, Grok DeepSearch, Manus, Genspark) — capabilities, disclosed technical details, and synthesized user sentiment from reviews, benchmarks, and discussion threads. Companion to [deep-research-fundamentals-2026-08-01.md](deep-research-fundamentals-2026-08-01.md) (general architecture).

---

## 1. Google Deep Research (Gemini)

**What it does:** Built into the Gemini app and, as of 2026, the Gemini API as a callable agent, running on **Gemini 3.1 Pro**. Before doing any work it produces a research *plan* and shows it to the user for confirmation/editing — a distinguishing UX choice versus competitors that just start searching. It then iteratively formulates queries, reads results, identifies "knowledge gaps," and re-searches until it judges coverage sufficient. Google introduced a two-tier split in 2026: **Deep Research** (fast/cheap, interactive) and **Deep Research Max** (extended test-time compute, async/background jobs, "maximum comprehensiveness"). Retrieval spans web search, remote MCP servers, uploaded files/connected file stores, and arbitrary custom tools/data repositories — the broadest declared retrieval surface of any product reviewed. Output can include native HTML/generated charts and infographics, not just text. Google's official materials disclose no iteration/search counts, no output-length spec, and gate pricing/rate limits behind paid tiers and enterprise agreements ([Google blog](https://blog.google/innovation-and-ai/models-and-research/gemini-models/next-generation-gemini-deep-research/)).

**Sentiment:** Praised for ecosystem integration (Gmail, Docs, Sheets, Drive) and multimodal breadth, much of it free with an AI Pro subscription. Consistent complaints: **slow**, especially reconciling contradicting sources; loses context in long threads, forcing prompt re-entry; still makes arithmetic mistakes and hallucinates on simple factual tasks. One reviewer said they hadn't personally seen citation hallucination but used it only "for background info, not specific citations" — an implicit admission that trust is conditional ([BuildFastWithAI review](https://www.buildfastwithai.com/ai-tools/gemini-deep-research)).

## 2. Anthropic Claude Research

**What it does:** A toggleable "Research" mode in Claude.ai, available Pro/Max/Team/Enterprise (Pro gets basic web search, not the full agentic loop, which needs Max/Team/Enterprise). Claude "operates agentically," issuing chained searches that build on each other, revising its plan when a source contradicts an earlier finding. As of mid-2026, Research runs **up to 45 minutes** across **"hundreds of internal and external sources,"** including connected apps/Google Workspace ([Anthropic/X](https://x.com/AnthropicAI/status/1917972753916797111)). Anthropic claims Claude tags findings by confidence tier — well-sourced, single-source (needs verification), or thin/contradictory evidence. Concrete numbers (search counts, output length, rate limits) aren't publicly documented; sessions just "use up your limits faster" ([Claude Help Center](https://support.claude.com/en/articles/11088861-use-research-on-claude)). Pricing: Pro $20/mo, Max $100–$200/mo.

**Sentiment:** Independent benchmarking found Claude Code-style agents scoring high (97%) on internal accuracy checkpoints and fastest wall-clock (~1.7 min) on coding-flavored research tasks at low cost (~$1.54/task) vs. OpenAI's o3 ($10.92/task, 8+ min). But general Claude Research is also described as using "fewer sources than competitors," favoring "document reasoning" over raw breadth. The most serious documented complaint is **citation fabrication**: independent 2025–2026 studies found roughly 15–20% fabricated citations on factual tasks, rising to 35–55% on niche/recent topics, with fake references carrying plausible author names and non-resolving DOIs; Anthropic's own outside counsel had to apologize to a court after Claude fabricated a legal citation in a filing ([VerusCite benchmark](https://www.crimrxiv.com/pub/s63si1hl/release/1)).

## 3. OpenAI Deep Research (ChatGPT)

**What it does:** Powered by a specialized version of **o3** "optimized for web browsing and real-world data analysis." Finds, reads, and synthesizes "hundreds of online sources" into an analyst-grade report, 5–30 minutes depending on complexity ([OpenAI](https://openai.com/index/introducing-deep-research/)). Independent benchmarking found o3-based deep research running **27–125 web searches per task** — the widest range among tools tested — at high cost (~$10.92/task) and long latency (~8.5 min), with **lower accuracy than cheaper competitors** (Perplexity Sonar and Claude/Parallel Ultra beat it on DR-50 and other checkpoints). Scored **26.6%** on Humanity's Last Exam at release, the highest of any model at the time. ChatGPT Plus ($20/mo) gets 25 deep-research queries/month; Pro tiers ($100–$200/mo) scale to ~125–250/month.

**Sentiment:** Frequently ranked **best for breadth and polish** — "longest, most polished narratives" — with a "notably low hallucination rate" *relative to* other deep-research tools, though OpenAI's own internal testing of the underlying o3/o4-mini found they hallucinate substantially more than the prior o1 generation on PersonQA (o3: 33%, more than double o1's rate) — good at deep research specifically, but the base reasoning model regressed on hallucination. Complaints center on speed and cost, and an aimultiple benchmark found a "word-count paradox" where verbose reports didn't reliably correlate with higher accuracy vs. terser competitor outputs.

## 4. Perplexity Deep Research

**What it does:** Decomposes the query into subtopics, runs 3–5 sequential reasoning/search loops, checks 50–100+ sources (vs. 10–20 in standard search), flags conflicting evidence, generates a structured narrative with source-confidence labels (high/medium/uncertain) and disputed-data flags. Runtime is the fastest of the majors: **2–4 minutes** vs. 20+ for ChatGPT, with **100–300 cited sources** typically shown vs. 20–50 for ChatGPT, and a "live process view" more transparent than OpenAI's largely black-box run ([datastudios.org](https://www.datastudios.org/post/perplexity-ai-deep-research-how-it-works-limitations-and-use-cases-for-professionals)). Perplexity co-published **DRACO**, an open cross-domain benchmark (academic, finance, law, medicine, technology) from millions of production research tasks, releasing rubrics and judge prompts publicly — a notable rigor signal ([arXiv](https://arxiv.org/pdf/2602.11685)). Pricing: Pro $20/mo, Enterprise up to ~$200/mo-equivalent.

**Sentiment:** Praised for speed and cost ($20/mo vs. ChatGPT Pro's $200/mo) and search-process transparency. Draws the sharpest hallucination criticism of the group: reviewers report it "confidently buries falsehoods within seemingly well-researched answers," with "2-3 to over half a dozen hallucinations" per query in some tests; TechRadar's head-to-head concluded it "doesn't quite live up to ChatGPT's research potential" on depth/nuance despite winning on speed. Early 2026 also saw "widespread Reddit and Trustpilot complaints" after **unannounced quota cuts** (some Pro daily allotments dropped from 250/day to ~20/month) — a reliability/trust complaint distinct from output quality ([aiqnahub.com](https://www.aiqnahub.com/perplexity-deep-research-not-worth-it/)).

## 5. Elicit

**What it does:** The scholarly-research specialist rather than a general web-research agent. Searches Semantic Scholar's 126M+-paper corpus, can fully automate a systematic literature review — generating PRISMA-style screening criteria, extracting quantitative/qualitative data (>90% claimed accuracy), synthesizing across up to 200 papers. Its defining differentiator: **sentence-level citation grounding** — every AI-generated claim ties to the exact source sentence/figure, not just a document-level link ([Elicit blog](https://elicit.com/blog/systematic-review/)). Claims up to 80% manual-time savings and ~95% recall identifying relevant papers.

**Sentiment:** The most consistently *positive* of the six — "great at finding relevant papers" though it "might miss some important ones" per a 2025 academic evaluation. Documented limitations: screening capped at 500 papers even with Zotero integration; searches aren't reproducible and lack transparency required for formal systematic-review reporting; content self-rated by reviewers at "80-90% accurate, definitely not 100%"; acknowledged English-language bias. Elicit's own team publishes its evaluation methodology rather than only marketing claims — unusually candid ([Elicit limitations](https://support.elicit.com/en/articles/549569)).

## 6. You.com Research, Grok DeepSearch, Manus, Genspark

**You.com** ships an agentic Research product plus a Research API with selectable effort tiers (lite/standard/deep/exhaustive/frontier). Its **ARI** claims to analyze up to 400 sources in under 5 minutes — "10x more sources, 3x faster" than unnamed competitors — outputting polished PDF reports, plus a Finance-specific Research API layered with licensed S&P Global data ([You.com](https://you.com/resources/introducing-ari-the-first-professional-grade-research-agent-for-business)). API/developer-first rather than consumer-chat-first, and largely absent from mainstream consumer comparison roundups.

**Grok DeepSearch/DeeperSearch** (xAI, on Grok 4.5 since July 2026) is the only major tool with live X/Twitter access alongside the open web — a real edge on breaking news and social-sentiment questions; priced via API at $2/$6 per million input/output tokens, 500K context window. **Manus** and **Genspark** are positioned as multi-agent "research + execution" tools (they can act on findings, not just report them); reviewers rate Genspark as "the closest direct competitor to Manus... often beats Manus on raw research quality" ([felloai.com](https://felloai.com/ai-search-deep-research-comparison/)).

---

## Where deep research is most useful vs. least useful

**Most useful:**
- **Literature/market surveys with a defined corpus** (Elicit's core case): closed, citable, largely-static source universe where "read 200 papers and summarize" is structurally what LLMs are good at, and sentence-level grounding makes verification tractable.
- **Breadth-first orientation tasks**: "give me the landscape of X" when the user has no starting knowledge and mild inaccuracy is an acceptable cost for a 10x speedup — where Perplexity's speed and Gemini's ecosystem breadth get consistent praise.
- **Structured, low-ambiguity aggregation**: public financial figures, published specs, regulatory filings — anywhere ground truth is a single authoritative document and the tool just needs to find and quote it.

**Least useful / consistently negative sentiment:**
- **Fast-moving news and true recency** — even Grok's X-integration advantage is narrow; web-search-based tools lag current events by crawl/index latency, and both Gemini and ChatGPT are markedly slower reconciling contradicting/recent sources.
- **Citation-level trust without human verification** — the single most consistently, universally reported failure: 11–57% citation hallucination rates reported across commercially deployed models in academic studies, 15–20% (up to 35–55% on niche/recent topics) specifically measured for Claude, and Perplexity users reporting several hallucinations per query buried confidently inside well-formatted answers.
- **Genuine analytical judgment / novel synthesis** — the most-repeated qualitative critique across independent (non-vendor) blogs is the "insight gap": these tools excel at aggregation, scale, and polish but rarely produce *new* understanding; heavy reliance was explicitly linked to erosion of the user's own critical-thinking, and training on the modal web corpus suppresses specialist/contrarian framings.
- **Long, contradictory, or highly technical verification tasks** — the aimultiple "word-count paradox" (a 5,253-word Perplexity report not outperforming a 248-word Codex answer) and failures to target the *correct version* of fast-changing technical docs show these tools struggle exactly where technical rigor matters most.

## What would beat all of these

Across every product the complaint pattern converges on one gap: **no product treats citation verification as a first-class, auditable pipeline stage rather than an emergent property of "the model was told to cite things."** Every vendor either self-reports low hallucination as a *relative* virtue or is silent on the number entirely; none publish a verification methodology as rigorous as what independent researchers (VerusCite, "Cited but Not Verified," DRACO) are now building externally. Concrete differentiator opportunities:

1. **A verification pass as a distinct, visible pipeline stage** — after synthesis, re-fetch every cited source, confirm the quoted claim actually appears in it (not just that the URL resolves), surface a per-claim confidence/verification badge in the final report. Elicit's sentence-level grounding is the closest analog but scoped to a closed academic corpus; nobody has shipped this at open-web scale with the same rigor.
2. **Reproducible, replayable research runs** — Elicit's own documented limitation ("searches are not directly reproducible") is universal across every product reviewed. Logging and replaying the exact search trace would be uniquely trustworthy for professional/regulated use and directly answers the "black box" complaint leveled at OpenAI.
3. **Effort-to-cost transparency without bait-and-switch quotas** — Perplexity's unannounced quota collapse generated some of the sharpest user backlash found in this research, worse in tone than any accuracy complaint. Stable, predictable rate limits earn durable trust that pure benchmark wins don't.
4. **Judgment-aware output, not just aggregation** — explicitly flagging where the literature is thin, contested, or the tool is extrapolating beyond its sources (rather than presenting synthesis and speculation in the same authoritative register) directly answers the most consistent qualitative critique — essentially productizing what Claude *claims* to do with confidence tiers, but rigorously enough that independent benchmarks would confirm rather than contradict the marketing claim.
5. **Recency-aware routing** — none of the general tools handle fast-moving/contradictory information well; routing time-sensitive sub-questions to live/streaming sources (as Grok routes to X) while routing stable factual sub-questions to a verified, cached corpus (as Elicit routes to Semantic Scholar) would beat every generalist on the failure mode users complain about most.

**For Vox specifically:** out-executing on iteration count, source count, or report length is a saturated, undifferentiated axis — nearly every competitor already claims "hundreds of sources" and "iterative refinement." The open competitive space is **verifiable trust infrastructure**: auditable citation-to-claim mapping, reproducible run logs, and honest confidence signaling — none of the six products reviewed has convincingly shipped this as of August 2026. This directly motivates prioritizing the trust/novelty-scoring work in [deep-research-trust-novelty-scoring-landscape-2026-08-01.md](deep-research-trust-novelty-scoring-landscape-2026-08-01.md) over pure breadth/speed improvements.

## Sources

- [Gemini Deep Research Agent — Google AI for Developers](https://ai.google.dev/gemini-api/docs/interactions/deep-research)
- [Build with Gemini Deep Research (Google blog)](https://blog.google/technology/developers/deep-research-agent-gemini-api/)
- [Deep Research Max: a step change for autonomous research agents (Google blog)](https://blog.google/innovation-and-ai/models-and-research/gemini-models/next-generation-gemini-deep-research/)
- [Gemini Deep Research — Google overview](https://gemini.google/overview/deep-research/)
- [Gemini Deep Research Review 2026 — BuildFastWithAI](https://www.buildfastwithai.com/ai-tools/gemini-deep-research)
- [Use research on Claude — Claude Help Center](https://support.claude.com/en/articles/11088861-use-research-on-claude)
- [Anthropic on X — Research mode 45 minutes announcement](https://x.com/AnthropicAI/status/1917972753916797111)
- [What's Going On with Claude Code? — alphaguruai Substack](https://alphaguruai.substack.com/p/whats-going-on-with-claude-code)
- [VerusCite V1 Benchmark: Citation Verification Accuracy — CrimRxiv](https://www.crimrxiv.com/pub/s63si1hl/release/1)
- [Introducing deep research — OpenAI](https://openai.com/index/introducing-deep-research/)
- [OpenAI's Deep Research Scores 26.6% on Humanity's Last Exam](https://chyshkala.com/blog/openai-s-deep-research-scores-26-6-on-humanity-s-last-exam-while-your-phd-takes-6-years)
- [OpenAI's leading models keep making things up — Tom's Guide](https://www.tomsguide.com/ai/openais-leading-models-keep-making-things-up-heres-why)
- [ChatGPT Deep Research pricing/queries — Prismer.ai](https://prismer.ai/blog/deep-research-queries-per-month-all-tools)
- [Perplexity AI Deep Research: How It Works, Limitations, and Use Cases — datastudios.org](https://www.datastudios.org/post/perplexity-ai-deep-research-how-it-works-limitations-and-use-cases-for-professionals)
- [DRACO: a Cross-Domain Benchmark for Deep Research (arXiv)](https://arxiv.org/pdf/2602.11685)
- [I tried Perplexity's Deep Research — TechRadar](https://www.techradar.com/computing/artificial-intelligence/i-tried-perplexitys-deep-research-and-it-doesnt-quite-live-up-to-chatgpts-research-potential)
- [Perplexity Deep Research Not Worth It in 2026? — aiqnahub.com](https://www.aiqnahub.com/perplexity-deep-research-not-worth-it/)
- [Elicit: AI for scientific research](https://elicit.com/)
- [Introducing Elicit Systematic Review — Elicit blog](https://elicit.com/blog/systematic-review/)
- [Elicit's limitations — Elicit support](https://support.elicit.com/en/articles/549569)
- [Comparison of Elicit AI and Traditional Literature Searching — medRxiv](https://www.medrxiv.org/content/10.1101/2025.06.17.25329772.full.pdf)
- [You.com — Introducing ARI](https://you.com/resources/introducing-ari-the-first-professional-grade-research-agent-for-business)
- [You.com Research API docs](https://you.com/docs/guides/research)
- [AI Search and Deep Research Tools Compared 2026 — felloai.com](https://felloai.com/ai-search-deep-research-comparison/)
- [Grok DeepSearch Review 2026 — buildfastwithai.com](https://www.buildfastwithai.com/ai-tools/grok-deepsearch)
- [AI Deep Research: Claude vs ChatGPT vs Grok — aimultiple.com](https://aimultiple.com/ai-deep-research)
- [Detecting and Correcting Reference Hallucinations in Commercial LLMs and Deep Research Agents (arXiv)](https://arxiv.org/html/2604.03173v1)
- [Cited but Not Verified: Parsing and Evaluating Source Attribution in LLM Deep Research Agents (arXiv)](https://arxiv.org/pdf/2605.06635)
- [AI Deep Research Flaw: Single Reddit Comment Steers Consumers to Scams — Tech Times](https://www.techtimes.com/articles/318839/20260622/ai-deep-research-flaw-single-reddit-comment-steers-consumers-scams.htm)
- [Deep Research isn't really deep research — AI Goes to College Substack](https://aigoestocollege.substack.com/p/deep-research-isnt-really-deep-research)
- [AI Deep Research: The Insight Gap — Medium/Data Science Collective](https://medium.com/data-science-collective/deep-research-in-ai-the-insight-gap-446118ebe76e)
- [Claude AI Pricing 2026 — ScreenApp](https://screenapp.io/blog/claude-ai-pricing)
