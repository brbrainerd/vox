// ── Statement/body builders ──────────────────────────────────────────────────

fn gen_body(rng: &mut Rng, ret_type: &str, complexity: u8, tags: &mut Vec<String>) -> String {
    let mut lines = Vec::new();
    let stmts = 1 + complexity as usize / 2;

    for _ in 0..stmts.min(4) {
        let (fname, ftype) = FIELD_POOL[rng.usize(FIELD_POOL.len())];
        let depth = (complexity / 3).min(2);
        let val = gen_expr(rng, depth, tags);
        if rng.coin() {
            lines.push(format!("    let {fname}: {ftype} = {val}"));
            tags.push("stmt:let".into());
        } else {
            lines.push(format!("    let {fname} = {val}"));
        }
    }

    if complexity >= 4 && rng.coin() {
        let cond = gen_binary(rng, 0);
        lines.push(format!("    if {cond} {{"));
        lines.push(format!(
            "        return {}",
            gen_literal_for_type(rng, ret_type)
        ));
        lines.push("    }".to_string());
        tags.push("expr:if".into());
    }

    if ret_type != "Unit" {
        lines.push(format!("    return {}", gen_literal_for_type(rng, ret_type)));
        tags.push("stmt:return".into());
    }

    lines.join("\n")
}

fn gen_params(rng: &mut Rng, count: usize) -> String {
    let mut params = Vec::new();
    let mut used = std::collections::HashSet::new();
    for _ in 0..count {
        let (name, ty) = loop {
            let entry = FIELD_POOL[rng.usize(FIELD_POOL.len())];
            if used.insert(entry.0) {
                break entry;
            }
        };
        params.push(format!("{name}: {ty}"));
    }
    params.join(", ")
}

fn gen_fields(rng: &mut Rng, count: usize) -> String {
    let mut lines = Vec::new();
    let mut used = std::collections::HashSet::new();
    for _ in 0..count {
        let (name, ty) = loop {
            let entry = FIELD_POOL[rng.usize(FIELD_POOL.len())];
            if used.insert(entry.0) {
                break entry;
            }
        };
        lines.push(format!("    {name}: {ty}"));
    }
    lines.join("\n")
}

// ── Organic pair ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct OrganicPair {
    pub prompt: String,
    pub response: String,
    pub category: String,
    pub verified: bool,
    pub complexity: u8,
    pub coverage_tags: Vec<String>,
}

impl OrganicPair {
    #[must_use]
    pub fn to_jsonl(&self) -> String {
        json!({
            "prompt": self.prompt,
            "response": self.response,
            "messages": [
                {"role": "user", "content": self.prompt},
                {"role": "assistant", "content": self.response}
            ],
            "category": self.category,
            "rating": if self.verified { 5 } else { 1 },
            "format": "vox_organic",
            "complexity": self.complexity,
            "coverage": self.coverage_tags,
            "schema_version": "vox_dogfood_v1",
        })
        .to_string()
    }
}

// ── Dynamic Decl generators (one per TAXONOMY entry) ─────────────────────────
// Each function maps a TAXONOMY_FROM_AST snake_case tag to a Vox source string.
// When a new Decl variant is added to vox-ast, TAXONOMY_FROM_AST grows, and the
// coverage report flags it as uncovered — prompting addition of a new generator.

fn generate_for_taxonomy_entry(tag: &str, rng: &mut Rng, variant: usize) -> Option<OrganicPair> {
    let noun = NOUNS[rng.usize(NOUNS.len())];
    let verb = VERBS[rng.usize(VERBS.len())];
    let name = format!("{verb}_{noun}");
    let type_name = {
        let mut s = String::from(&noun[..1].to_uppercase());
        s.push_str(&noun[1..]);
        s
    };
    let ret_type = gen_return_type(rng, 1);
    let param_count = 1 + variant % 3;
    let params = gen_params(rng, param_count);
    let complexity = 2 + (variant % 5) as u8;
    let mut tags = vec![format!("decl:{tag}")];

    let (source, prompt) = match tag {
        "function" => {
            let body = gen_body(rng, &ret_type, complexity, &mut tags);
            let dec = ["", "@async\n", "@traced\n", "@pure\n"][variant % 4];
            (
                format!("{dec}fn {name}({params}) to {ret_type} {{\n{body}\n}}"),
                format!("Write a Vox function called `{name}` that returns `{ret_type}`"),
            )
        }
        "component" => {
            tags.push("expr:jsx".into());
            let jsx = format!(
                "    view: <div className=\"{noun}\">\n        <h1>{{\"{type_name}\"}}</h1>\n    </div>"
            );
            (
                format!("component {type_name}View({params}) {{\n{jsx}\n}}"),
                format!("Create a Vox UI component called `{type_name}View`"),
            )
        }
        "reactive_component" => {
            tags.push("expr:jsx".into());
            let (sf, st) = FIELD_POOL[rng.usize(FIELD_POOL.len())];
            let mount_cleanup = "    on mount {\n        // Initialize data\n    }\n    on cleanup {\n        // teardown\n    }".to_string();
            (
                format!(
                    "component {type_name}({params}) {{\n    state {sf}: {st} = {}\n{}\n    view: <div className=\"{noun}\">\n        <h1>{{\"{type_name}\"}}</h1>\n        <p>{{{sf}}}</p>\n    </div>\n}}",
                    gen_literal_for_type(rng, st),
                    mount_cleanup
                ),
                format!("Create a modern reactive Vox component called `{type_name}`"),
            )
        }
        "actor" => {
            let handler_count = 1 + variant % 3;
            let (sf, st) = FIELD_POOL[rng.usize(FIELD_POOL.len())];
            let mut handlers = Vec::new();
            for i in 0..handler_count {
                let ev = &VERBS[(rng.usize(VERBS.len()) + i) % VERBS.len()];
                let hr = gen_prim_type(rng);
                handlers.push(format!(
                    "    on {ev}() to {hr} {{\n        {sf} = {sf} + 1\n        return {}\n    }}",
                    gen_literal_for_type(rng, &hr)
                ));
            }
            (
                format!(
                    "actor {type_name}Actor {{\n    state {sf}: {st} = {}\n\n{}\n}}",
                    gen_literal_for_type(rng, st),
                    handlers.join("\n\n")
                ),
                format!(
                    "Define a Vox actor called `{type_name}Actor` with {handler_count} handlers"
                ),
            )
        }
        "workflow" => {
            let body = gen_body(rng, &ret_type, complexity, &mut tags);
            (
                format!("workflow {name}({params}) to {ret_type} {{\n{body}\n}}"),
                format!("Write a durable Vox workflow called `{name}`"),
            )
        }
        "activity" => {
            let body = gen_body(rng, &ret_type, complexity, &mut tags);
            (
                format!("activity {name}({params}) to {ret_type} {{\n{body}\n}}"),
                format!("Define a Vox activity called `{name}`"),
            )
        }
        "table" => {
            let fc = 2 + variant % 4;
            let fields = gen_fields(rng, fc);
            (
                format!("@table type {type_name} {{\n{fields}\n}}"),
                format!("Define a Vox @table schema `{type_name}` with {fc} fields"),
            )
        }
        "http_route" => {
            let methods = ["get", "post", "put", "delete"];
            let m = methods[variant % methods.len()];
            (
                format!("@{m}(\"/api/{noun}\")\nfn {name}(req: str) to str {{\n    return \"ok\"\n}}"),
                format!("Create a Vox HTTP {m} handler at `/api/{noun}`"),
            )
        }
        "mcp_tool" => (
            format!(
                "@mcp.tool \"{name}: {verb} data\"\nfn {name}({params}) to str {{\n    return \"done\"\n}}"
            ),
            format!("Define a Vox MCP tool called `{name}`"),
        ),
        "mcp_resource" => (
            format!(
                "@mcp.resource \"{noun}://{{path}}\"\nfn read_{noun}(path: str) to str {{\n    return path\n}}"
            ),
            format!("Define a Vox MCP resource for `{noun}`"),
        ),
        "query" => (
            format!(
                "@query\nfn get_{noun}(id: int) to str {{\n    let result = db.{type_name}.find(id)\n    return result\n}}"
            ),
            format!("Write a Vox @query fn to read from `{type_name}`"),
        ),
        "mutation" => (
            format!(
                "@mutation\nfn update_{noun}(id: int, value: str) to Unit {{\n    db.{type_name}.update(id, value)\n}}"
            ),
            format!("Write a Vox @mutation fn to write to `{type_name}`"),
        ),
        "action" => {
            let body = gen_body(rng, &ret_type, complexity, &mut tags);
            (
                format!("@action\nfn {name}({params}) to {ret_type} {{\n{body}\n}}"),
                format!("Write a Vox @action called `{name}`"),
            )
        }
        "test" => (
            format!(
                "@test\nfn test_{noun}() to Unit {{\n    let result = {verb}(42)\n    assert(result > 0)\n}}"
            ),
            format!("Write a Vox @test for `{verb}`"),
        ),
        "type_def" => {
            let src = match variant % 3 {
                0 => format!("type {type_name}Status = Active | Inactive | Pending"),
                1 => format!("type {type_name}Result = Success(data: str) | Error(msg: str)"),
                _ => format!("type {type_name}Option[T] = Some(value: T) | None"),
            };
            (src, format!("Define a Vox union type for `{type_name}`"))
        }
        "import" => {
            let modules = ["std.json", "network.HTTP", "db.users"];
            (
                format!("import {}", modules[variant % modules.len()]),
                "Write a Vox import statement".into(),
            )
        }
        "message" => {
            let fields = gen_fields(rng, 2 + variant % 3);
            (
                format!("message {type_name}Event {{\n{fields}\n}}"),
                format!("Define a Vox inter-agent message `{type_name}Event`"),
            )
        }
        "scheduled" => {
            let intervals = ["1h", "30m", "24h", "5m"];
            let iv = intervals[variant % intervals.len()];
            (
                format!(
                    "@scheduled(\"{iv}\")\nfn {name}_job() to Unit {{\n    let status = check()\n    log(status)\n}}"
                ),
                format!("Create a Vox scheduled job running every {iv}"),
            )
        }
        "server_fn" => {
            let body = gen_body(rng, &ret_type, complexity, &mut tags);
            (
                format!("@server\nfn {name}({params}) to {ret_type} {{\n{body}\n}}"),
                format!("Write a Vox @server fn called `{name}`"),
            )
        }
        "const" => {
            let ty = gen_prim_type(rng);
            (
                format!(
                    "const {}_LIMIT: {ty} = {}",
                    noun.to_uppercase(),
                    gen_literal_for_type(rng, &ty)
                ),
                format!("Declare a Vox constant of type `{ty}`"),
            )
        }
        "collection" => {
            let fields = gen_fields(rng, 3);
            (
                format!(
                    "@collection type {type_name}Doc {{\n{fields}\n    embedding: list[float]\n}}"
                ),
                format!("Define a Vox @collection for `{type_name}`"),
            )
        }
        "index" => {
            let (f, _) = FIELD_POOL[rng.usize(FIELD_POOL.len())];
            (
                format!("@index {type_name}.by_{f} on ({f})"),
                format!("Define a Vox @index on `{type_name}.{f}`"),
            )
        }
        "vector_index" => (
            format!(
                "@vector_index {type_name}Doc.by_embedding on (embedding) {{ dimensions: 768 }}"
            ),
            format!("Define a Vox @vector_index for `{type_name}Doc`"),
        ),
        "search_index" => (
            format!("@search_index {type_name}Doc.by_content on (title, description)"),
            format!("Define a Vox @search_index on `{type_name}Doc`"),
        ),
        "trait" => (
            format!(
                "trait {type_name}Trait {{\n    fn {verb}(self) to str\n    fn validate(self) to bool\n}}"
            ),
            format!("Define a Vox trait `{type_name}Trait`"),
        ),
        "impl" => (
            format!(
                "impl Serializable for {type_name} {{\n    fn serialize(self) to str {{\n        return \"{noun}\"\n    }}\n}}"
            ),
            format!("Implement a trait for `{type_name}`"),
        ),
        "skill" => (
            format!("@skill\nfn {name}_skill({params}) to str {{\n    return \"analyzed\"\n}}"),
            format!("Define a Vox @skill called `{name}_skill`"),
        ),
        "agent_def" => (
            format!(
                "@agent_def\nfn {name}_agent() to str {{\n    tools: [{verb}]\n    memory: long_term\n    return \"ready\"\n}}"
            ),
            format!("Define a Vox @agent_def `{name}_agent`"),
        ),
        "agent" => {
            let (sf, st) = FIELD_POOL[rng.usize(FIELD_POOL.len())];
            (
                format!(
                    "agent {type_name}Agent {{\n    state {sf}: {st} = {}\n    on {verb}() to str {{\n        return \"processed\"\n    }}\n}}",
                    gen_literal_for_type(rng, st)
                ),
                format!("Define a Vox agent `{type_name}Agent`"),
            )
        }
        "config" => {
            let fields = gen_fields(rng, 3);
            (
                format!("config {type_name}Config {{\n{fields}\n}}"),
                format!("Define a Vox config block `{type_name}Config`"),
            )
        }
        "context" => (
            format!(
                "context {type_name}Context {{\n    value: str\n    update: fn(str) to Unit\n}}"
            ),
            format!("Define a Vox context `{type_name}Context`"),
        ),
        "hook" => (
            format!(
                "hook fn use_{noun}(initial: int) to (int, fn() -> Unit) {{\n    let state = initial\n    return (state, fn() to state + 1)\n}}"
            ),
            format!("Define a Vox hook `use_{noun}`"),
        ),
        "provider" => (
            format!(
                "provider fn {type_name}Provider(children: Element) to Element {{\n    return <div>{{children}}</div>\n}}"
            ),
            format!("Define a Vox provider `{type_name}Provider`"),
        ),
        "fixture" => (
            format!("@fixture\nfn setup_{noun}() to str {{\n    return \"test_fixture\"\n}}"),
            format!("Define a Vox @fixture for `{noun}`"),
        ),
        "layout" => (
            format!(
                "layout fn {type_name}Layout(children: Element) to Element {{\n    return <main>{{children}}</main>\n}}"
            ),
            format!("Define a Vox layout `{type_name}Layout`"),
        ),
        "loading" => (
            format!(
                "loading fn {type_name}Loading() to Element {{\n    return <div>{{\"Loading...\"}}</div>\n}}"
            ),
            format!("Define a Vox loading component for `{type_name}`"),
        ),
        "not_found" => (
            format!(
                "not_found fn {type_name}NotFound() to Element {{\n    return <h1>{{\"404 - Not Found\"}}</h1>\n}}"
            ),
            format!("Define a Vox 404 handler for `{type_name}`"),
        ),
        "error_boundary" => (
            format!(
                "error_boundary fn {type_name}Error(error: str) to Element {{\n    return <div class=\"error\">{{error}}</div>\n}}"
            ),
            format!("Define a Vox error boundary for `{type_name}`"),
        ),
        "keyframes" => (
            format!(
                "@keyframes {noun}_fade {{\n    from {{ opacity: 0 }}\n    to {{ opacity: 1 }}\n}}"
            ),
            format!("Define Vox @keyframes `{noun}_fade`"),
        ),
        "theme" => (
            format!(
                "theme dark_{noun} {{\n    bg: \"#1a1a2e\"\n    fg: \"#e0e0e0\"\n    accent: \"#00d4ff\"\n}}"
            ),
            format!("Define a Vox dark theme for `{noun}`"),
        ),
        "mock" => (
            format!("@mock\nfn mock_{noun}() to str {{\n    return \"mock_data\"\n}}"),
            format!("Define a Vox @mock for `{noun}`"),
        ),
        "environment" => (
            format!(
                "environment {noun}_staging {{\n    region: \"us-east-1\"\n    replicas: 2\n    debug: false\n}}"
            ),
            format!("Define a Vox environment for `{noun}`"),
        ),
        "page" => (
            format!(
                "page fn {type_name}Page() to Element {{\n    return <section>\n        <h1>{{\"{type_name}\"}}</h1>\n    </section>\n}}"
            ),
            format!("Define a Vox static page `{type_name}Page`"),
        ),
        "routes" => (
            format!(
                "routes {{\n    \"/\" to {type_name}Page\n    \"/{noun}\" to {type_name}View\n    \"/{noun}/:id\" to {type_name}Detail\n}}"
            ),
            format!("Define Vox routes for `{type_name}`"),
        ),
        "v0_component" => (
            format!("component {type_name}Widget() {{\n    // @v0(\"https://v0.dev/t/example\")\n    view: <div></div>\n}}"),
            format!("Define a Vox v0.dev component `{type_name}Widget`"),
        ),
        _ => {
            // Unknown taxonomy entry — generate a generic function tagged with the construct
            let body = gen_body(rng, "str", complexity, &mut tags);
            (
                format!("# {tag} construct\nfn {name}({params}) to str {{\n{body}\n}}"),
                format!("Write a Vox `{tag}` construct called `{name}`"),
            )
        }
    };

    // Ensure the SFT training explicitly flags older React/TS artifacts (if generation paths emit raw TSX) 
    // Currently, everything should now be native Vox.
    Some(OrganicPair {
        prompt,
        response: source,
        category: format!("vox_{tag}"),
        verified: false,
        complexity,
        coverage_tags: tags,
    })
}
