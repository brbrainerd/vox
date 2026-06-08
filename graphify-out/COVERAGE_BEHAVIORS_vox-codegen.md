# Semantic Behavior Map — `vox-codegen`

Deterministically synthesized from 385 distinct proven-behavior claims (of 385 extracted) across 223 symbols. 26 symbols have an explicit error-path proof; **165 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `resolve()`  (edge, error, happy; EXTRACTED)
- [happy] resolve() emits margin classes in Tailwind format (e.g., 'mb-2', 'mt-4') when margin kwargs are provided  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve() converts token-shaped color values (e.g., 'zinc.400') to Tailwind format with dash separator and prefix (e.g., 'text-zinc-400')  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve() emits 'bg-' prefixed Tailwind classes for background token values on panel primitives  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve() emits distinct rounded classes for radius and radius_br (corner-specific) attributes  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve() emits 'max-w-' and 'min-h-' prefixed Tailwind classes for size attributes  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve() emits 'flex-' and 'shrink-' prefixed Tailwind classes for flexbox layout attributes  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve() emits 'uppercase' Tailwind class for case='upper' attribute on text primitives  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve() emits 'tracking-' and 'leading-' prefixed Tailwind classes for text layout attributes  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve() emits 'justify-' and 'items-' prefixed Tailwind flexbox classes for alignment attributes  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve() preserves raw_class values verbatim without transformation (e.g., 'border-white/10 backdrop-blur-md')  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [edge] resolve() does not consume unknown kwargs; they pass through unchanged to allow typos to surface as HTML attributes  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] heading primitive without explicit level argument defaults to h2 HTML tag  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- … +9 more claims

### `generate`  (happy; EXTRACTED)
- [happy] Form emission exports a React component with the form name  (crates/vox-codegen/tests/form_emit_test.rs)
- [happy] Form emission includes declared field labels in the component  (crates/vox-codegen/tests/form_emit_test.rs)
- [happy] Integer fields in forms emit number input type  (crates/vox-codegen/tests/form_emit_test.rs)
- [happy] Form emission awaits endpoint function call on submit  (crates/vox-codegen/tests/form_emit_test.rs)
- [happy] Form success redirect uses router-agnostic history.pushState and popstate event  (crates/vox-codegen/tests/form_emit_test.rs)
- [happy] Declared error_message text is rendered in the emitted form component  (crates/vox-codegen/tests/form_emit_test.rs)
- [happy] Golden example .vox files generate valid TypeScript code  (crates/vox-codegen/tests/golden_ts_test.rs)
- [happy] Golden example .vox files pass typeck validation  (crates/vox-codegen/tests/golden_ts_test.rs)
- [happy] Golden example TypeScript output matches insta snapshots  (crates/vox-codegen/tests/golden_ts_test.rs)
- [happy] List renders with explicit key parameter emit the key prop in generated TypeScript code  (crates/vox-codegen/tests/list_keys_test.rs)
- [happy] codegen_ts generate function emits a fragments.tsx file when module contains fragment declarations  (crates/vox-codegen/src/codegen_ts/fragment_emit.rs)
- [happy] generated fragments.tsx includes GreetingArgs interface and function definition for declared fragments  (crates/vox-codegen/src/codegen_ts/fragment_emit.rs)

### `emit_state_machine_decls`  (happy; EXTRACTED)
- [happy] Returns empty string when module contains no state machine declarations  (crates/vox-codegen/src/codegen_ts/state_machine_emit.rs)
- [happy] Emits TypeScript type declaration 'export type Light =' for state machine named Light  (crates/vox-codegen/src/codegen_ts/state_machine_emit.rs)
- [happy] Includes string literal tags for state names (On, Off) in discriminated union type  (crates/vox-codegen/src/codegen_ts/state_machine_emit.rs)
- [happy] Emits TypeScript event type declaration 'export type LightEvent =' with event names as string literals  (crates/vox-codegen/src/codegen_ts/state_machine_emit.rs)
- [happy] Emits reducer function with signature 'export function lightReducer(state: Light, event: LightEvent)'  (crates/vox-codegen/src/codegen_ts/state_machine_emit.rs)
- [happy] Imports useState and useCallback from react module for state machine hook  (crates/vox-codegen/src/codegen_ts/state_machine_emit.rs)
- [happy] Emits React hook function 'export function useLightStateMachine(initial: Light)' alongside reducer  (crates/vox-codegen/src/codegen_ts/state_machine_emit.rs)
- [happy] Hook includes send method with signature '(event: LightEvent) => void'  (crates/vox-codegen/src/codegen_ts/state_machine_emit.rs)
- [happy] Event handler is wrapped with useCallback to memoize the dispatch function  (crates/vox-codegen/src/codegen_ts/state_machine_emit.rs)
- [happy] Hook calls lightReducer(prev, event) to transition state based on dispatched events  (crates/vox-codegen/src/codegen_ts/state_machine_emit.rs)

### `emit_fn`  (happy, invariant; EXTRACTED)
- [happy] Emitted Rust code for activity functions contains the journal::execute runtime call  (crates/vox-codegen/tests/durability_compiles.rs)
- [happy] Workflow function lowering emits the interpret_workflow_durable call in Rust code  (crates/vox-codegen/tests/durability_lowering.rs)
- [happy] Activity function lowering emits the journal::execute call in Rust code  (crates/vox-codegen/tests/durability_lowering.rs)
- [happy] Actor function lowering emits the vox_actor_runtime::spawn_process call in Rust code  (crates/vox-codegen/tests/durability_lowering.rs)
- [happy] Actor dispatch table emits fully-wired Envelope routing with no placeholder dispatch markers  (crates/vox-codegen/tests/durability_lowering.rs)
- [happy] Actor dispatch table emits handler names in routing matches (e.g., MyActor_greet, MyActor_tick)  (crates/vox-codegen/tests/durability_lowering.rs)
- [invariant] Non-durable plain functions do not emit workflow runtime interpret_workflow_durable call  (crates/vox-codegen/tests/durability_lowering.rs)
- [invariant] Non-durable plain functions do not emit journal::execute call  (crates/vox-codegen/tests/durability_lowering.rs)
- [invariant] Non-durable plain functions do not emit actor runtime vox_actor_runtime::spawn_process call  (crates/vox-codegen/tests/durability_lowering.rs)

### `emit_main`  (happy, invariant; EXTRACTED)
- [happy] emit_main includes set_current_hir_module call for HIR registration  (crates/vox-codegen/tests/emit_main_includes_durable_boot.rs)
- [happy] emit_main includes load_hir_module_from_embedded helper for durability support  (crates/vox-codegen/tests/emit_main_includes_durable_boot.rs)
- [happy] emit_main registers scheduled functions via scheduled::register call  (crates/vox-codegen/tests/emit_main_includes_durable_boot.rs)
- [happy] emit_main starts the scheduler via scheduled::start call  (crates/vox-codegen/tests/emit_main_includes_durable_boot.rs)
- [happy] emit_main creates a distinct vox_durable_db binding separate from Codex db  (crates/vox-codegen/tests/emit_main_includes_durable_boot.rs)
- [happy] emit_main still emits HTTP server wiring via axum::serve when durable boot is added  (crates/vox-codegen/tests/emit_main_includes_durable_boot.rs)
- [happy] emit_main includes HIR registration via set_current_hir_module even without @scheduled functions  (crates/vox-codegen/tests/emit_main_includes_durable_boot.rs)
- [happy] emit_main includes load_hir_module_from_embedded helper even without @scheduled functions  (crates/vox-codegen/tests/emit_main_includes_durable_boot.rs)
- [invariant] emit_main does not start the scheduler when no @scheduled functions are present  (crates/vox-codegen/tests/emit_main_includes_durable_boot.rs)

### `resolve`  (happy; EXTRACTED)
- [happy] resolve("stack") returns an element with html_tag "div" and base_classes containing both "flex" and "flex-col"  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve("stack") with gap attribute includes the corresponding gap class (e.g., "gap-4" for gap="4")  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve("row") returns an element with html_tag "div" and base_classes containing both "flex-row" and "flex-wrap"  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve("row") with wrap="false" includes "flex-nowrap" class and excludes "flex-wrap" class  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve("row") with scroll="horizontal" includes "flex-nowrap" and "overflow-x-auto" classes but excludes "flex-wrap"  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve("text") returns an element with html_tag "p"  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve("text") with size="sm" includes "text-sm" class in base_classes  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] resolve("heading") with level="1" returns an element with html_tag "h1" and base_classes containing "text-3xl"  (crates/vox-codegen/src/web_ir/primitives/mod.rs)

### `emit_from_contract`  (happy, invariant; EXTRACTED)
- [happy] emit_from_contract output is valid JSON parseable by serde_json  (crates/vox-codegen/src/codegen_ts/openapi_emit.rs)
- [happy] emit_from_contract generates OpenAPI 3.1.0 spec with empty paths when given empty ContractIr  (crates/vox-codegen/src/codegen_ts/openapi_emit.rs)
- [happy] emit_from_contract transforms Decimal field type to string type with x-vox-encoding decimal marker in OpenAPI schema  (crates/vox-codegen/src/codegen_ts/openapi_emit.rs)
- [happy] emit_from_contract emits sum types as OpenAPI oneOf with _tag discriminator property  (crates/vox-codegen/src/codegen_ts/openapi_emit.rs)
- [happy] emit_from_contract generates GET operation with query parameters and JSON encoding hint description  (crates/vox-codegen/src/codegen_ts/openapi_emit.rs)
- [happy] emit_from_contract includes ErrorEnvelope component schema and references it in 400 and default error responses  (crates/vox-codegen/src/codegen_ts/openapi_emit.rs)
- [invariant] emit_from_contract generates OpenAPI paths without duplicating /api segment when server URL is empty  (crates/vox-codegen/src/codegen_ts/openapi_emit.rs)

### `emit_tokens_css`  (happy, invariant; EXTRACTED)
- [happy] CSS output emits annotated token values as hex without object syntax  (crates/vox-codegen/src/codegen_ts/tokens_emit.rs)
- [happy] Generates CSS custom property --vox-color-primary with value #3a86ff  (crates/vox-codegen/src/codegen_ts/tokens_emit.rs)
- [happy] Generates CSS custom property --vox-color-background with value #ffffff  (crates/vox-codegen/src/codegen_ts/tokens_emit.rs)
- [happy] Generates CSS custom property --vox-color-text with value #1d3557  (crates/vox-codegen/src/codegen_ts/tokens_emit.rs)
- [happy] Generates CSS custom property --vox-spacing-md with value 16px  (crates/vox-codegen/src/codegen_ts/tokens_emit.rs)
- [invariant] CSS output excludes $schema key from token registry  (crates/vox-codegen/src/codegen_ts/tokens_emit.rs)
- [invariant] CSS output does not leak schema-related text into generated properties  (crates/vox-codegen/src/codegen_ts/tokens_emit.rs)

### `RoutePattern overlap detection`  (edge, happy; EXTRACTED)
- [edge] Two identical literal paths return Overlap::Ambiguous  (crates/vox-codegen/src/codegen_ts/route_pattern.rs)
- [happy] Completely disjoint literal paths return Overlap::None  (crates/vox-codegen/src/codegen_ts/route_pattern.rs)
- [edge] A literal segment shadows a parameter segment at the same position, returning Overlap::Shadowed  (crates/vox-codegen/src/codegen_ts/route_pattern.rs)
- [edge] Two parameter routes with different parameter names return Overlap::Ambiguous  (crates/vox-codegen/src/codegen_ts/route_pattern.rs)
- [happy] Routes with parameters but disjoint literal segments (different prefix) return Overlap::None  (crates/vox-codegen/src/codegen_ts/route_pattern.rs)
- [happy] Routes with different numbers of segments and no wildcard return Overlap::None  (crates/vox-codegen/src/codegen_ts/route_pattern.rs)

### `emit_hir_expr_attr_value`  (happy; EXTRACTED)
- [happy] emits await for async endpoint calls in let-binding assignments within handlers  (crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs)
- [happy] emits await with parentheses for async endpoint calls as method chain receivers  (crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs)
- [happy] emits await for async endpoint calls in variable assignments within handlers  (crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs)
- [happy] does not emit async keyword or await for handlers with only synchronous calls  (crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs)
- [happy] emits lambda-wrapped handlers as single async arrow function without double-wrapping when containing async calls  (crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs)
- [happy] awaits async endpoint calls in match scrutinee position to discriminate on resolved values not promises  (crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs)

### `validate_web_ir`  (error, happy, invariant; EXTRACTED)
- [happy] validate_web_ir returns no web_ir_validate.route.missing_component diagnostic when route references component listed in view_roots  (crates/vox-codegen/src/web_ir/validate.rs)
- [error] validate_web_ir returns web_ir_validate.route.missing_component diagnostic when route references component not in view_roots  (crates/vox-codegen/src/web_ir/validate.rs)
- [invariant] validate_web_ir does not emit web_ir_validate.route.unreachable diagnostic for root / route  (crates/vox-codegen/src/web_ir/validate.rs)
- [happy] validate_web_ir does not emit web_ir_validate.route.unreachable diagnostic when route is reachable via link element with to attribute  (crates/vox-codegen/src/web_ir/validate.rs)
- [error] validate_web_ir emits web_ir_validate.route.unreachable diagnostic for non-root route without any link elements  (crates/vox-codegen/src/web_ir/validate.rs)
- [error] validate_web_ir emits web_ir_validate.route.unreachable diagnostic when link element is not reachable from any view root (orphan)  (crates/vox-codegen/src/web_ir/validate.rs)

### `attrs()`  (edge, happy; EXTRACTED)
- [happy] attrs() correctly encodes margin keyword arguments for downstream processing  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] attrs() correctly encodes flex and shrink layout parameters for row primitives  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] attrs() correctly encodes tracking and leading typography parameters  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] attrs() correctly encodes justify and items alignment parameters for row primitives  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [edge] attrs() preserves unknown keyword arguments for attribute passthrough without silent truncation  (crates/vox-codegen/src/web_ir/primitives/mod.rs)

### `borrowable_params()`  (error, happy; EXTRACTED)
- [error] A parameter used inside an object literal argument is not marked as borrowable  (crates/vox-codegen/src/codegen_rust/emit/param_borrow.rs)
- [error] A parameter used in a binary operator is not marked as borrowable  (crates/vox-codegen/src/codegen_rust/emit/param_borrow.rs)
- [error] A parameter bound in a let statement is not marked as borrowable  (crates/vox-codegen/src/codegen_rust/emit/param_borrow.rs)
- [error] A parameter used in a compound call argument is not marked as borrowable  (crates/vox-codegen/src/codegen_rust/emit/param_borrow.rs)
- [happy] An unused parameter is included in the borrowable parameters set  (crates/vox-codegen/src/codegen_rust/emit/param_borrow.rs)

### `emit_route_id_module`  (happy, invariant; EXTRACTED)
- [happy] Route ID module emits constructor functions for each route  (crates/vox-codegen/src/web_ir/href_emit.rs)
- [happy] Route ID module exports hrefOf helper function  (crates/vox-codegen/src/web_ir/href_emit.rs)
- [happy] Route ID module exports analyticsSlugOf helper function  (crates/vox-codegen/src/web_ir/href_emit.rs)
- [happy] hrefOf uses template literal expressions for route parameter substitution  (crates/vox-codegen/src/web_ir/href_emit.rs)
- [invariant] analyticsSlugOf returns distinct slug values per route  (crates/vox-codegen/src/web_ir/href_emit.rs)

### `emit_route_path_builder()`  (edge, happy; EXTRACTED)
- [happy] Generates KnownRoute type union including all route patterns provided  (crates/vox-codegen/src/codegen_ts/route_manifest.rs)
- [happy] Emits typed builder function with string parameter for parameterized routes (e.g., /users/:id)  (crates/vox-codegen/src/codegen_ts/route_manifest.rs)
- [happy] Emits identity function (no parameters) for literal routes without path parameters  (crates/vox-codegen/src/codegen_ts/route_manifest.rs)
- [edge] Wildcard routes are exposed as identity function returning literal pattern unchanged  (crates/vox-codegen/src/codegen_ts/route_manifest.rs)
- [edge] Returns empty string when given empty route list  (crates/vox-codegen/src/codegen_ts/route_manifest.rs)

### `expo_router_fs_path`  (happy; EXTRACTED)
- [happy] expo_router_fs_path converts path parameter notation (e.g., :id) to bracket notation (e.g., [id])  (crates/vox-codegen/src/codegen_ts/rn/routes.rs)
- [happy] expo_router_fs_path correctly transforms multiple nested route parameters from colon notation to bracket notation  (crates/vox-codegen/src/codegen_ts/rn/routes.rs)
- [happy] expo_router_fs_path converts wildcard route segments (*) to Expo Router catchall notation ([...rest])  (crates/vox-codegen/src/codegen_ts/rn/routes.rs)
- [happy] converts root route path '/' to 'index' filesystem path for expo-router  (crates/vox-codegen/src/codegen_ts/rn/routes.rs)
- [happy] preserves single-segment route paths unchanged in expo-router filesystem conversion  (crates/vox-codegen/src/codegen_ts/rn/routes.rs)

### `RoutePattern::overlap_with`  (happy; EXTRACTED)
- [happy] Returns Overlap::Shadowed when wildcard pattern /files/* is compared with a more-specific literal pattern /files/readme.md  (crates/vox-codegen/src/codegen_ts/route_pattern.rs)
- [happy] Returns Overlap::Shadowed when wildcard pattern /files/* is compared with a more-specific param pattern /files/:name  (crates/vox-codegen/src/codegen_ts/route_pattern.rs)
- [happy] Returns Overlap::Ambiguous for two identical wildcard patterns /*  (crates/vox-codegen/src/codegen_ts/route_pattern.rs)
- [happy] Returns Overlap::Ambiguous when comparing root patterns / and empty string (both represent root)  (crates/vox-codegen/src/codegen_ts/route_pattern.rs)

### `emit_error_boundary`  (happy, invariant; EXTRACTED)
- [happy] ErrorBoundary is a React class component extending React.Component  (crates/vox-codegen/src/codegen_ts/web_entry.rs)
- [happy] ErrorBoundary implements getDerivedStateFromError lifecycle method  (crates/vox-codegen/src/codegen_ts/web_entry.rs)
- [happy] ErrorBoundary implements componentDidCatch lifecycle method  (crates/vox-codegen/src/codegen_ts/web_entry.rs)
- [invariant] ErrorBoundary does not include app-specific IndexedDB or crash-log coupling  (crates/vox-codegen/src/codegen_ts/web_entry.rs)

### `emit_fragment_decls`  (edge, happy; EXTRACTED)
- [edge] emit_fragment_decls returns empty string when HirModule has no fragments  (crates/vox-codegen/src/codegen_ts/fragment_emit.rs)
- [happy] emit_fragment_decls generates zero-argument function signature export function Greeting(): React.ReactElement for paramless fragments  (crates/vox-codegen/src/codegen_ts/fragment_emit.rs)
- [happy] emit_fragment_decls output starts with vox-compiler header comment when fragments exist  (crates/vox-codegen/src/codegen_ts/fragment_emit.rs)
- [happy] emit_fragment_decls includes import React from react when fragments exist  (crates/vox-codegen/src/codegen_ts/fragment_emit.rs)

### `emit_url_decls`  (edge, happy; EXTRACTED)
- [happy] Emits union type with tagged variant for simple URL declarations  (crates/vox-codegen/src/codegen_ts/url_emit.rs)
- [happy] Emits typed fields and builder signatures for parameterized URL variants  (crates/vox-codegen/src/codegen_ts/url_emit.rs)
- [happy] Optional parameters in URL variants emit as optional TypeScript fields  (crates/vox-codegen/src/codegen_ts/url_emit.rs)
- [edge] Returns empty string when source contains no URL declarations  (crates/vox-codegen/src/codegen_ts/url_emit.rs)

### `emit_web_app`  (edge, happy; EXTRACTED)
- [happy] Router app emits manifest import and VoxApp export without external router dependencies  (crates/vox-codegen/src/codegen_ts/web_entry.rs)
- [happy] Flat app mounts root component via ES import  (crates/vox-codegen/src/codegen_ts/web_entry.rs)
- [happy] Flat app fills required component props with placeholder values  (crates/vox-codegen/src/codegen_ts/web_entry.rs)
- [edge] Empty app mode exports VoxApp without manifest reference  (crates/vox-codegen/src/codegen_ts/web_entry.rs)

### `emit_web_entry`  (happy; EXTRACTED)
- [happy] Generated web entry imports runtime-install module  (crates/vox-codegen/src/codegen_ts/web_entry.rs)
- [happy] Generated web entry imports VoxApp component from vox-app module  (crates/vox-codegen/src/codegen_ts/web_entry.rs)
- [happy] Generated web entry imports wrapApp and onBoot from app-hooks module  (crates/vox-codegen/src/codegen_ts/web_entry.rs)
- [happy] Generated web entry mounts VoxApp to DOM root element and calls onBoot  (crates/vox-codegen/src/codegen_ts/web_entry.rs)

### `validate_a11y_with_registry function with text contrast`  (error, happy, invariant; EXTRACTED)
- [happy] text element with sufficient contrast ratio (>4.5:1 for body text) produces no diagnostic errors  (crates/vox-codegen/src/web_ir/validate_a11y.rs)
- [error] text element with contrast ratio below 3:1 emits web_ir_validate.a11y.insufficient_contrast diagnostic  (crates/vox-codegen/src/web_ir/validate_a11y.rs)
- [error] body text element with contrast ratio between 3:1 and 4.5:1 emits web_ir_validate.a11y.low_contrast warning  (crates/vox-codegen/src/web_ir/validate_a11y.rs)
- [invariant] text element without ancestor data-vox-surface context produces no contrast diagnostic errors  (crates/vox-codegen/src/web_ir/validate_a11y.rs)

### `Panel()`  (happy; EXTRACTED)
- [happy] Panel() primitive supports background attribute resolution with token values  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] Panel() primitive supports both generic radius and corner-specific radius_br attributes  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] Panel() primitive supports max_w and min_h attributes for size constraints  (crates/vox-codegen/src/web_ir/primitives/mod.rs)

### `RoutePattern parsing`  (happy; EXTRACTED)
- [happy] Literal path segments like /users/me parse into a vector of Segment::Literal variants with correct names  (crates/vox-codegen/src/codegen_ts/route_pattern.rs)
- [happy] Parameter segments like :id parse into Segment::Param variants with the parameter name  (crates/vox-codegen/src/codegen_ts/route_pattern.rs)
- [happy] Wildcard segments like * parse into a Segment::Wildcard variant  (crates/vox-codegen/src/codegen_ts/route_pattern.rs)

### `emit_app_hooks_default`  (happy; EXTRACTED)
- [happy] Default app hooks imports ErrorBoundary from vox-error-boundary module  (crates/vox-codegen/src/codegen_ts/web_entry.rs)
- [happy] Default app hooks imports and registers service worker  (crates/vox-codegen/src/codegen_ts/web_entry.rs)
- [happy] Default app hooks wraps application in ErrorBoundary component  (crates/vox-codegen/src/codegen_ts/web_entry.rs)

### `emit_async_view_tsx`  (happy; EXTRACTED)
- [happy] Emitted async view discriminates on _tag field for all four async states  (crates/vox-codegen/src/web_ir/async_state.rs)
- [happy] Emitted async view extracts error and value from async discriminated union  (crates/vox-codegen/src/web_ir/async_state.rs)
- [happy] Emitted async view renders all four arm components for fetching, empty, error, and success states  (crates/vox-codegen/src/web_ir/async_state.rs)

### `emit_ident_expr()`  (happy; EXTRACTED)
- [happy] A borrowed str identifier emits as .as_str() method call  (crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs)
- [happy] A borrowed non-str identifier emits as & reference, not .as_str()  (crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs)
- [happy] An owned non-Copy identifier emits with .clone() method call  (crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs)

### `emit_openclaw_or_browser_registry_call()`  (happy; EXTRACTED)
- [happy] Scrape namespace calls emit to vox_scrape_* runtime symbols without wasm32 guard  (crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs)
- [happy] Scrape.select_attr with three arguments emits with .as_str() conversion applied to each  (crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs)
- [happy] Browser namespace calls are protected by wasm32 conditional compilation guard  (crates/vox-codegen/src/codegen_rust/emit/stmt_expr.rs)

### `emit_sw_register`  (happy; EXTRACTED)
- [happy] Service worker registration exports async function registerServiceWorker  (crates/vox-codegen/src/codegen_ts/web_entry.rs)
- [happy] Service worker registration checks for navigator.serviceWorker support  (crates/vox-codegen/src/codegen_ts/web_entry.rs)
- [happy] Service worker registration logs errors instead of throwing  (crates/vox-codegen/src/codegen_ts/web_entry.rs)

### `generate_rust_output`  (happy, invariant; EXTRACTED)
- [invariant] generate_rust_output produces identical output across multiple runs for the same input  (crates/vox-codegen/tests/property_tests.rs)
- [happy] generate_rust_output does not emit compile_error! macro calls for valid snippets  (crates/vox-codegen/tests/property_tests.rs)
- [happy] generate_rust_output does not emit todo!() calls for valid snippets  (crates/vox-codegen/tests/property_tests.rs)

### `validate_a11y function with button element`  (error, happy; EXTRACTED)
- [error] button element without text child or aria-label emits web_ir_validate.a11y.interactive_missing_label diagnostic  (crates/vox-codegen/src/web_ir/validate_a11y.rs)
- [happy] button element with text child content produces no diagnostic errors  (crates/vox-codegen/src/web_ir/validate_a11y.rs)
- [happy] button element with aria-label attribute produces no diagnostic errors  (crates/vox-codegen/src/web_ir/validate_a11y.rs)

### `emit_main_boot snapshot`  (edge, invariant; EXTRACTED)
- [invariant] emit_main_boot output with scheduled functions, actors, and server endpoints matches the snapshot  (crates/vox-codegen/tests/main_boot_snapshot.rs)
- [edge] emit_main_boot output for a module with no scheduled functions or endpoints matches the snapshot  (crates/vox-codegen/tests/main_boot_snapshot.rs)

### `emit_reactive_modules()`  (edge, happy; EXTRACTED)
- [edge] Returns empty result when given empty HirModule  (crates/vox-codegen/src/codegen_ts/reactive_module_emit.rs)
- [happy] Emits exactly one file with provider interface, context, provider component, and useState hook  (crates/vox-codegen/src/codegen_ts/reactive_module_emit.rs)

### `emit_sitemap_xml`  (happy; EXTRACTED)
- [happy] Emitted sitemap XML includes loc entries for all routes  (crates/vox-codegen/src/web_ir/href_emit.rs)
- [happy] Emitted sitemap is valid XML with version declaration  (crates/vox-codegen/src/web_ir/href_emit.rs)

### `is_primitive()`  (error, invariant; EXTRACTED)
- [invariant] is_primitive() recognizes exactly 10 primitives: stack, row, column, text, button, link, panel, card, list, route_outlet  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [error] is_primitive() returns false for generic HTML tags like 'div' and 'span'  (crates/vox-codegen/src/web_ir/primitives/mod.rs)

### `process.exit`  (error, happy; EXTRACTED)
- [happy] process.exit(1) lowers to std::process::exit  (crates/vox-codegen/tests/null_and_namespace_lowering.rs)
- [error] process.exit does not emit a method call on an undefined process variable  (crates/vox-codegen/tests/null_and_namespace_lowering.rs)

### `text()`  (happy; EXTRACTED)
- [happy] text() primitive supports color token resolution with dash-separated syntax  (crates/vox-codegen/src/web_ir/primitives/mod.rs)
- [happy] text() primitive supports text transformation via case attribute  (crates/vox-codegen/src/web_ir/primitives/mod.rs)

### `valid_for_target`  (happy; EXTRACTED)
- [happy] valid_for_target returns true for @mantine/core on web target and false on react-native target  (crates/vox-codegen/src/codegen_ts/external_libs.rs)
- [happy] valid_for_target returns true for react-native-paper on react-native target and false on web target  (crates/vox-codegen/src/codegen_ts/external_libs.rs)

### `validate_a11y`  (error, happy; EXTRACTED)
- [error] validate_a11y emits web_ir_validate.a11y.img_missing_alt diagnostic for img element without alt attribute  (crates/vox-codegen/src/web_ir/validate_a11y.rs)
- [happy] validate_a11y emits no diagnostics for img element with alt attribute  (crates/vox-codegen/src/web_ir/validate_a11y.rs)

### `validate_a11y function with anchor element`  (error; EXTRACTED)
- [error] anchor element without href emits web_ir_validate.a11y.anchor_missing_href diagnostic and skips interactive_missing_label check  (crates/vox-codegen/src/web_ir/validate_a11y.rs)
- [error] anchor element with href but no accessible name emits web_ir_validate.a11y.interactive_missing_label diagnostic  (crates/vox-codegen/src/web_ir/validate_a11y.rs)

### `validate_a11y function with role="button" element`  (error, happy; EXTRACTED)
- [error] element with role="button" without keyboard event handler emits web_ir_validate.a11y.role_button_missing_keyboard diagnostic  (crates/vox-codegen/src/web_ir/validate_a11y.rs)
- [happy] element with role="button" and keydown event handler produces no role_button_missing_keyboard diagnostic  (crates/vox-codegen/src/web_ir/validate_a11y.rs)

### `validate_contract_branch()`  (error; EXTRACTED)
- [error] Detects when route references loader function not declared as @query and reports error  (crates/vox-codegen/src/codegen_ts/route_manifest.rs)
- [error] Flags missing pending component referenced in route metadata  (crates/vox-codegen/src/codegen_ts/route_manifest.rs)

### `validate_manifest_symbols`  (error, happy; EXTRACTED)
- [error] When WebIrModule has routes referencing components not defined in HirModule, validation returns Err with the missing component name  (crates/vox-codegen/src/codegen_ts/route_manifest.rs)
- [happy] When no route_nodes are declared in WebIrModule, validation passes successfully  (crates/vox-codegen/src/codegen_ts/route_manifest.rs)

### `web_config_files`  (invariant; EXTRACTED)
- [invariant] Emitted scaffold package.json does not contain react-router in dependencies section  (crates/vox-codegen/src/codegen_ts/scaffold.rs)
- [invariant] Emitted scaffold package.json does not contain react-router in devDependencies section  (crates/vox-codegen/src/codegen_ts/scaffold.rs)

### `@back_button declaration code generation`  (happy; EXTRACTED)
- [happy] TypeScript emit includes voxRuntime.onBackButton() call and @vox/runtime adapter import for @back_button declaration  (crates/vox-codegen/tests/back_button_test.rs)

### `@query endpoint import in on-mount handlers`  (happy; EXTRACTED)
- [happy] A @query function called within an on-mount handler emits an import declaration 'import { fn_name } from "./vox-client"'  (crates/vox-codegen/tests/component_import_refs_test.rs)

### `@vox/runtime import deduplication`  (invariant; EXTRACTED)
- [invariant] when both @back_button and @deep_link are present, the @vox/runtime import appears exactly once (deduplicated)  (crates/vox-codegen/tests/deep_link_test.rs)

### `ASYNC_TYPE_ALIAS`  (invariant; EXTRACTED)
- [invariant] Async type alias includes all four discriminant tags: fetching, empty, error, ok  (crates/vox-codegen/src/web_ir/async_state.rs)

### `AssetManifest icon file validation`  (happy; EXTRACTED)
- [happy] AssetManifest.validate_preflight() succeeds and stage_under() copies icon files to staging directory  (crates/vox-codegen/src/assets/mod.rs)

### `Backdrop()`  (happy; INFERRED)
- [happy] Backdrop class values can be passed through raw_class attribute without prefix transformation  (crates/vox-codegen/src/web_ir/primitives/mod.rs)

### `BuiltinRegistry inlining for std.time.now_ms`  (happy; EXTRACTED)
- [happy] BuiltinRegistry.standard().lookup_function("std.time.now_ms", 0) returns BuiltinLowering::Inline("Date.now()") for direct JS expansion  (crates/vox-codegen/tests/builtin_registry_test.rs)

### `BuiltinRegistry lookup for str.length`  (happy; EXTRACTED)
- [happy] BuiltinRegistry.standard().lookup_method("str", "length", 0) returns BuiltinLowering::Property("length") variant  (crates/vox-codegen/tests/builtin_registry_test.rs)

### `BuiltinRegistry namespace preservation`  (happy; EXTRACTED)
- [happy] BuiltinRegistry.standard().lookup_namespace("Speech") returns the canonical name "Speech" (not remapped to alternate like 'mobile')  (crates/vox-codegen/tests/builtin_registry_test.rs)

### `CodegenOptions strict_ai=false`  (happy; EXTRACTED)
- [happy] TypeScript emit records missing-ts-ai-lowering diagnostic when AI fixture present with strict_ai disabled  (crates/vox-codegen/tests/ai_fixture_ts_diagnostic.rs)

### `CodegenOptions strict_ai=true`  (error; EXTRACTED)
- [error] TypeScript codegen fails with missing-ts-ai-lowering error when AI fixture present with strict_ai enabled  (crates/vox-codegen/tests/ai_fixture_ts_diagnostic.rs)

### `Codex database management in tables-only Tauri apps`  (happy; EXTRACTED)
- [happy] Tables-only Tauri app.manage() the Codex database in generated code  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `CounterProvider`  (happy; EXTRACTED)
- [happy] Generated provider component uses PascalCase naming (CounterProvider.tsx for counter module)  (crates/vox-codegen/src/codegen_ts/reactive_module_emit.rs)

### `ErrorEnvelope schema`  (happy; EXTRACTED)
- [happy] ErrorEnvelope schema contains an ok property with const value false  (crates/vox-codegen/tests/openapi_crud_api_test.rs)

### `File-based speech transcribe without microphone capability`  (happy; EXTRACTED)
- [happy] Speech.transcribe(file_path) derives speech capability but not microphone in capability_ids  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `HirModule embedding in Tauri main`  (happy; EXTRACTED)
- [happy] Generates src-tauri/src/main.rs calling load_hir_module_from_embedded to embed and register HirModule  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `HirModule serialization roundtrip`  (invariant; EXTRACTED)
- [invariant] a HirModule serialized to JSON and deserialized preserves function count, endpoint count, and scheduled intervals  (crates/vox-codegen/tests/main_boot_hir_roundtrip.rs)

### `LayerTier`  (invariant; EXTRACTED)
- [invariant] tier_z_index returns strictly ascending values where Modal > Popover, Toast > Modal, and SystemOverlay > Toast  (crates/vox-codegen/src/web_ir/layer_emit.rs)

### `Library mode codegen`  (happy; EXTRACTED)
- [happy] Library mode generates a package.json with exports for openapi.json  (crates/vox-codegen/tests/openapi_crud_api_test.rs)

### `Link element import`  (happy; EXTRACTED)
- [happy] link element import from expo-router is included in RN component output  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `MyButton component render`  (happy; EXTRACTED)
- [happy] imported external React component renders as component tag in output  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `PaperProvider guidance emission`  (happy; EXTRACTED)
- [happy] react-native-paper import triggers provider guidance emission in output  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `RustAppShell::AxumLocalServer code generation`  (happy; EXTRACTED)
- [happy] Generates src/main.rs containing a rust_app_shell marker with value AxumLocalServer  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `RustAppShell::AxumLocalServer code generation with AI fixture`  (happy; EXTRACTED)
- [happy] Generated Rust crate bundle from Vox code passes cargo check successfully  (crates/vox-codegen/tests/ai_fixture_bundle_compiles.rs)

### `RustAppShell::TauriApp code generation`  (happy; EXTRACTED)
- [happy] Generates src-tauri/src/main.rs containing a rust_app_shell marker with value TauriApp  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `STT plugin emission when Speech namespace is used`  (happy; EXTRACTED)
- [happy] Modules using Speech.transcribe_microphone() emit vox_tauri_stt::plugin::init() in Tauri main.rs  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `STT plugin omission when Speech is not used`  (happy; EXTRACTED)
- [happy] Empty module does not emit vox_tauri_stt in Tauri main.rs, build.rs, capabilities/default.json, or Cargo.toml  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `Scheduler omission for tables-only modules`  (happy; EXTRACTED)
- [happy] Tables-only Tauri app does not register a scheduler (no scheduled::register call)  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `ScrollView import`  (happy; EXTRACTED)
- [happy] ScrollView is imported from React Native when row scroll attributes are used  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `Tauri InlinedPlugin ACL generation when speech is used`  (happy; EXTRACTED)
- [happy] Speech usage in Tauri generates src-tauri/build.rs with InlinedPlugin::new() and vox-stt plugin ID  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `Tauri api_client_ts emptiness`  (happy; EXTRACTED)
- [happy] Tauri app generation leaves api_client_ts empty; vox-client.ts is SSOT  (crates/vox-codegen/tests/tauri_endpoint_client_parity_test.rs)

### `Tauri capability manifest generation`  (happy; EXTRACTED)
- [happy] Generates src-tauri/capabilities/default.json with vox-stt:default permission when speech is used  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `Tauri command attribute`  (happy; EXTRACTED)
- [happy] #[tauri::command] attribute appears in generated Tauri main.rs  (crates/vox-codegen/tests/tauri_endpoint_client_parity_test.rs)

### `Tauri command function generation`  (happy; EXTRACTED)
- [happy] @query function lowers to async fn with tauri::command attribute in main.rs  (crates/vox-codegen/tests/tauri_endpoint_client_parity_test.rs)

### `Tauri crate excludes web server dependencies`  (happy; EXTRACTED)
- [happy] Tauri src-tauri/Cargo.toml does not contain axum or rust-embed dependencies  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `Tauri plugin dependency with correct relative path`  (happy; EXTRACTED)
- [happy] Cargo.toml under src-tauri uses ../../../crates/ path for plugin dependencies (vox-tauri-stt)  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `Tauri setup block generation for tables-only modules`  (happy; EXTRACTED)
- [happy] A module with only @table declarations emits a .setup() block in Tauri main.rs  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `Tauri shell generation`  (happy; EXTRACTED)
- [happy] generate() produces RustAppShell::TauriApp with src-tauri/src/main.rs and src-tauri/build.rs files  (crates/vox-codegen/tests/tauri_convergence_snapshots.rs)

### `WebIR view roots for endpoint-only module`  (happy; EXTRACTED)
- [happy] endpoint-only module produces empty WebIR view_roots  (crates/vox-codegen/tests/tauri_endpoint_client_parity_test.rs)

### `WebIrModule`  (happy; EXTRACTED)
- [happy] validate_web_ir returns empty diagnostic list for default WebIrModule  (crates/vox-codegen/src/web_ir/mod.rs)

### `Workspace Cargo.toml excludes web server when using Tauri`  (happy; EXTRACTED)
- [happy] Tauri workspace Cargo.toml root does not list axum dependency  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `activity lowering transformation`  (happy; EXTRACTED)
- [happy] activity functions lower to journal::execute wrapper and reference activity_id in emitted Rust code  (crates/vox-codegen/tests/durability_lowering.rs)

### `activity runtime symbol resolution`  (happy; EXTRACTED)
- [happy] activity emit references ::vox_workflow_runtime::journal::execute in emitted Rust  (crates/vox-codegen/tests/durability_compiles.rs)

### `actor dispatch routing`  (happy; EXTRACTED)
- [happy] actor shell emits real Envelope dispatch table with message/request routing, handler names as string literals, and ProcessContext::reply calls, removing no-op dispatch markers  (crates/vox-codegen/tests/durability_lowering.rs)

### `actor lowering transformation`  (happy; EXTRACTED)
- [happy] actor declarations lower to vox_actor_runtime::spawn_process calls in emitted Rust code  (crates/vox-codegen/tests/durability_lowering.rs)

### `async IIFE`  (happy; EXTRACTED)
- [happy] awaited async endpoint calls run in an async IIFE  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `async endpoint call`  (happy; EXTRACTED)
- [happy] async endpoint calls in on-mount handlers are awaited  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `bare_package extraction for scoped subpaths`  (happy; EXTRACTED)
- [happy] bare_package(@mui/material/Button) returns Some(@mui/material)  (crates/vox-codegen/src/codegen_ts/external_libs.rs)

### `bare_package for relative paths`  (happy; EXTRACTED)
- [happy] bare_package returns None for relative paths like ./Foo.tsx or ../ui/Bar  (crates/vox-codegen/src/codegen_ts/external_libs.rs)

### `bare_package for unscoped subpaths`  (happy; EXTRACTED)
- [happy] bare_package(react-aria/useButton) returns Some(react-aria)  (crates/vox-codegen/src/codegen_ts/external_libs.rs)

### `bleed attribute`  (happy; EXTRACTED)
- [happy] screen roots with bleed=true opt out of default screen padding  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `class_string()`  (happy; EXTRACTED)
- [happy] class_string() method on resolved stack primitive joins CSS classes with space, containing 'flex' and 'flex-col'  (crates/vox-codegen/src/web_ir/primitives/mod.rs)

### `collect_table_select_projections`  (happy; EXTRACTED)
- [happy] collects SELECT projections from HirDbQueryPlan within endpoint function bodies  (crates/vox-codegen/src/codegen_rust/emit/tables/projections.rs)

### `component self-import prevention`  (invariant; EXTRACTED)
- [invariant] A component file does NOT emit a self-import (e.g., Solo.tsx does not contain 'import { Solo } from "./Solo"')  (crates/vox-codegen/tests/component_import_refs_test.rs)

### `css injection for styled libraries`  (happy; EXTRACTED)
- [happy] An auto-injected CSS import 'import "@mantine/core/styles.css";' is emitted when a CSS-dependent library (@mantine/core) is imported  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `deep-link runtime adapter emission`  (happy; EXTRACTED)
- [happy] deep-link configuration emits voxRuntime.onDeepLink(), handler call, useDeepLinkRouting hook, useEffect import, and @vox/runtime adapter import  (crates/vox-codegen/tests/deep_link_test.rs)

### `deep_link handler runtime adapter integration`  (happy; EXTRACTED)
- [happy] A @deep_link declaration emits calls to the vox runtime adapter (voxRuntime.onDeepLink) and imports the useDeepLinkRouting hook from @vox/runtime  (crates/vox-codegen/tests/deep_link_test.rs)

### `deep_link useEffect import`  (happy; EXTRACTED)
- [happy] A @deep_link declaration results in an import of useEffect from React for lifecycle integration  (crates/vox-codegen/tests/deep_link_test.rs)

### `default.json excludes microphone permission for file transcribe`  (invariant; EXTRACTED)
- [invariant] File-based transcribe capabilities/default.json does not contain microphone (case-insensitive) token  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `direct argument reads included in dep array`  (happy; EXTRACTED)
- [happy] When a derived binding directly references state in the argument passed to a function, that state is included in the dep array even if the function is non-@reactive  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `discarded endpoint call`  (happy; EXTRACTED)
- [happy] discarded endpoint calls do not emit dead const _ = promise bindings  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `emit_cargo_toml`  (happy; EXTRACTED)
- [happy] emit_cargo_toml includes rust_imports (crate names and versions) in the generated full-app Cargo.toml  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `emit_dialog_primitive`  (happy; EXTRACTED)
- [happy] emit_dialog_primitive generates TSX containing role=dialog, aria-modal=true, aria-label binding, Tab key handler, focus restoration, and Escape handler  (crates/vox-codegen/src/web_ir/semantic_ui_emit.rs)

### `emit_fn for numeric addition`  (happy; EXTRACTED)
- [happy] emit_fn emits clean `1 + 2` without spurious RHS borrow (`1 + &2`)  (crates/vox-codegen/tests/binary_op_emit.rs)

### `emit_fn with @ai task_category and strengths attributes`  (happy; EXTRACTED)
- [happy] emit_fn maps task_category and strengths attributes to LlmConfig telemetry fields  (crates/vox-codegen/tests/ai_structured_output_emit.rs)

### `emit_fn with @ai(structured_output=...)`  (happy; EXTRACTED)
- [happy] emit_fn wires response_format through LlmConfig when structured_output is declared, including schema name  (crates/vox-codegen/tests/ai_structured_output_emit.rs)

### `emit_fn with @prompt fixture`  (happy; EXTRACTED)
- [happy] emit_fn emits cascade_for_research_stage construction with matching ResearchStage when @prompt fixture present  (crates/vox-codegen/tests/ai_structured_output_emit.rs)

### `emit_fn with @search fixture`  (happy; EXTRACTED)
- [happy] emit_fn references lookup_fact_by_key and emits SearchDispatch telemetry for @search fixture  (crates/vox-codegen/tests/ai_structured_output_emit.rs)

### `emit_fn with @subagent(policy=distributed)`  (happy; EXTRACTED)
- [happy] emit_fn calls relay_ai_fixture_distributed_subagent and gates with populi-transport cfg feature  (crates/vox-codegen/tests/ai_structured_output_emit.rs)

### `emit_fn with @subagent(policy=parallel)`  (happy; EXTRACTED)
- [happy] emit_fn creates DispatchRouter and calls route_with_telemetry for parallel subagent policy  (crates/vox-codegen/tests/ai_structured_output_emit.rs)

### `emit_fn without structured_output`  (happy; EXTRACTED)
- [happy] emit_fn omits response_format injection when structured_output is not declared  (crates/vox-codegen/tests/ai_structured_output_emit.rs)

### `emit_hir_expr`  (happy; EXTRACTED)
- [happy] emits all branches of nested JSX if-expressions as nested ternaries without void IIFEs  (crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs)

### `emit_layer_type_alias`  (invariant; EXTRACTED)
- [invariant] Layer type alias includes string literal for all defined tiers  (crates/vox-codegen/src/web_ir/layer_emit.rs)

### `emit_main route registration`  (happy; EXTRACTED)
- [happy] emit_main generates .route() calls with GET for Query endpoints and POST for Mutation endpoints, with matching async handler functions  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `emit_main_boot() generates valid JSON`  (happy; EXTRACTED)
- [happy] emit_main_boot generates a raw string EMBEDDED_HIR containing valid JSON that deserializes to HirModule  (crates/vox-codegen/tests/main_boot_hir_roundtrip.rs)

### `emit_mobile_setup`  (happy; EXTRACTED)
- [happy] shell_projection_round_trip produces emit_mobile_setup output that matches the snapshot  (crates/vox-codegen/tests/mobile_emit_test.rs)

### `emit_runtime_install`  (happy; EXTRACTED)
- [happy] Runtime installation exposes global functions and test transcript hook  (crates/vox-codegen/src/codegen_ts/web_entry.rs)

### `emit_table_struct`  (happy; EXTRACTED)
- [happy] emit_table_struct emits a field pub _id: Option<i64> for table structures  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `emit_table_struct Option field handling`  (happy; EXTRACTED)
- [happy] emit_table_struct generates from_row code that deserializes Option<int> fields using row.get::<Option<i64>>()  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `emit_table_struct SQL generation`  (happy; EXTRACTED)
- [happy] emit_table_struct generates SQL FROM, INSERT INTO, and DELETE FROM clauses using lowercase table names  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `emit_table_struct bool field handling`  (happy; EXTRACTED)
- [happy] emit_table_struct generates from_row code that deserializes boolean fields using row.get::<i64>() with != 0 check  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `emit_table_struct find method`  (happy; EXTRACTED)
- [happy] emit_table_struct generates find method returning Result<Self, turso::Error> to enforce non-null policy and error when records are missing  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `emit_table_struct projection support`  (happy; EXTRACTED)
- [happy] emit_table_struct emits from_row_sel_* and all_proj_* methods with explicit column lists in SELECT SQL when projections are configured  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `emit_tabs_primitive`  (happy; EXTRACTED)
- [happy] emit_tabs_primitive generates TSX containing role=tablist, aria-selected attribute, role=tab, role=tabpanel, and aria-labelledby  (crates/vox-codegen/src/web_ir/semantic_ui_emit.rs)

### `emit_tokens_css and emit_tokens_ts`  (invariant; EXTRACTED)
- [invariant] CSS and TS token emission produces identical output regardless of input key insertion order  (crates/vox-codegen/src/codegen_ts/tokens_emit.rs)

### `emit_tokens_ts`  (happy; EXTRACTED)
- [happy] TypeScript output exports voxTokens const with 'as const' and VoxTokenKey type  (crates/vox-codegen/src/codegen_ts/tokens_emit.rs)

### `emit_with()`  (happy; EXTRACTED)
- [happy] With emission produces execute_activity_result() call without panic mapping, preserving stable activity identity  (crates/vox-codegen/src/codegen_rust/emit/with_emit.rs)

### `empty dep array for non-reactive callees without direct state reads`  (edge; EXTRACTED)
- [edge] When a derived binding calls a non-@reactive function with no state arguments, the useMemo dependency array is empty ('useMemo(() => opaque(), [])')  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `empty module HirModule roundtrip`  (edge; EXTRACTED)
- [edge] an empty HirModule with only a plain function serializes and deserializes correctly with endpoint_fns being empty  (crates/vox-codegen/tests/main_boot_hir_roundtrip.rs)

### `endpoint call transformation`  (happy; EXTRACTED)
- [happy] positional multi-arg endpoint calls are rewritten to named-object form matching vox-client signature, and positional form is eliminated  (crates/vox-codegen/tests/component_import_refs_test.rs)

### `endpoint positional call form suppression`  (error; EXTRACTED)
- [error] Positional endpoint call syntax (e.g., record_event("mood", "{}")) does NOT survive in generated code (prevented to ensure runtime argument binding works)  (crates/vox-codegen/tests/component_import_refs_test.rs)

### `endpoint positional-to-named argument rewriting`  (happy; EXTRACTED)
- [happy] Multi-argument endpoint function calls (e.g., record_event("mood", "{}")) are rewritten to named-argument object form (record_event({ kind: "mood", payload: "{}" })) to match vox-client.ts endpoint function signature  (crates/vox-codegen/tests/component_import_refs_test.rs)

### `error arm lowering`  (happy; EXTRACTED)
- [happy] Result-returning builtins emit Err(m) for error arms, not the undefined Error(m) constructor  (crates/vox-codegen/tests/null_and_namespace_lowering.rs)

### `error envelope code field`  (happy; EXTRACTED)
- [happy] error_envelope.example.json has non-empty code string field  (crates/vox-codegen/tests/wire_format_golden.rs)

### `error envelope details field`  (happy; EXTRACTED)
- [happy] error_envelope.example.json has details field present  (crates/vox-codegen/tests/wire_format_golden.rs)

### `error envelope message field`  (happy; EXTRACTED)
- [happy] error_envelope.example.json has non-empty message string field  (crates/vox-codegen/tests/wire_format_golden.rs)

### `error envelope ok field`  (happy; EXTRACTED)
- [happy] error_envelope.example.json has ok field set to false  (crates/vox-codegen/tests/wire_format_golden.rs)

### `error envelope request_id field`  (happy; EXTRACTED)
- [happy] error_envelope.example.json has request_id field present  (crates/vox-codegen/tests/wire_format_golden.rs)

### `external React import`  (happy; EXTRACTED)
- [happy] external React component import emits ES6 import statement not sibling import  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `external react component tag rendering`  (happy; EXTRACTED)
- [happy] An external React component is rendered as a JSX component tag (<MyButton) in the emitted code  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `extract_state_deps`  (happy; EXTRACTED)
- [happy] extracts state identifiers from binary expression operands  (crates/vox-codegen/src/codegen_ts/hir_emit/state_deps.rs)

### `extract_state_deps_with_callees`  (happy; EXTRACTED)
- [happy] recursively descends into reactive callee bodies to find state dependencies not visible at call site  (crates/vox-codegen/src/codegen_ts/hir_emit/state_deps.rs)

### `fire-and-forget endpoint`  (happy; EXTRACTED)
- [happy] fire-and-forget endpoint calls are wrapped with .catch() to prevent unhandled promise rejections  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `generate_script`  (happy; EXTRACTED)
- [happy] generate_script emits a Cargo.toml with tracing dependency when log.* builtins are lowered  (crates/vox-codegen/tests/null_and_namespace_lowering.rs)

### `generate_script_with_target Cargo.toml generation`  (happy; EXTRACTED)
- [happy] generate_script_with_target includes rust_imports (crate names and versions) in the generated Cargo.toml for script targets  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `generate_script_with_target for Wasi`  (happy; EXTRACTED)
- [happy] generate_script_with_target succeeds (returns Ok) for empty modules compiled to Wasi target  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `grouped named imports for external components`  (happy; EXTRACTED)
- [happy] Named imports are emitted as a grouped import statement 'import { Dialog, DialogContent } from "@radix-ui/react-dialog";'  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `html primitive import suppression`  (error; EXTRACTED)
- [error] HTML primitives like 'panel' do not generate import statements  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `html primitive text import suppression`  (error; EXTRACTED)
- [error] HTML primitives like 'text' do not generate import statements  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `import statement generation for cross-component usage`  (happy; EXTRACTED)
- [happy] When a component Inner is referenced within another component Outer's view, an ES import statement 'import { Inner } from "./Inner";' is emitted in the Outer.tsx output  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `import()`  (happy; EXTRACTED)
- [happy] Badge component import is emitted and detected inside conditional branch (if/else) in generated code  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `is null lowering`  (happy; EXTRACTED)
- [happy] Vox 'is null' operator on an Option type lowers to Rust .is_none() method call  (crates/vox-codegen/tests/null_and_namespace_lowering.rs)

### `isnt null lowering`  (happy; EXTRACTED)
- [happy] Vox 'isnt null' operator on an Option type lowers to Rust .is_some() method call  (crates/vox-codegen/tests/null_and_namespace_lowering.rs)

### `key() emission`  (happy; EXTRACTED)
- [happy] emitting TypeScript for a list render with a field key produces output containing 'key={w}' or 'key='  (crates/vox-codegen/tests/list_keys_test.rs)

### `link href attribute conversion`  (happy; EXTRACTED)
- [happy] link(href=...) source converts to <Link href={{...}}> JSX in output  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `link label wrapping in Text element`  (happy; EXTRACTED)
- [happy] link label content is wrapped in <Text> element for RN compatibility  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `list render validation`  (error; EXTRACTED)
- [error] list render without an explicit key fails with a validation error containing 'validate.list_key.required' or 'key'  (crates/vox-codegen/tests/list_keys_test.rs)

### `lookup resolves @mantine/core styling`  (happy; EXTRACTED)
- [happy] lookup(@mantine/core) returns library with Styling::CssFile  (crates/vox-codegen/src/codegen_ts/external_libs.rs)

### `lookup resolves @mui/material subpath to bare package`  (happy; EXTRACTED)
- [happy] lookup(@mui/material/Button) returns library with package @mui/material  (crates/vox-codegen/src/codegen_ts/external_libs.rs)

### `lookup returns None for relative paths`  (happy; EXTRACTED)
- [happy] lookup(./local) returns None for relative paths  (crates/vox-codegen/src/codegen_ts/external_libs.rs)

### `lookup returns None for unknown packages`  (happy; EXTRACTED)
- [happy] lookup(totally-unknown-pkg) returns None  (crates/vox-codegen/src/codegen_ts/external_libs.rs)

### `named component tag rendering`  (happy; EXTRACTED)
- [happy] Named external React components are rendered as JSX component tags (<Dialog, <DialogContent)  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `named import grouping`  (happy; EXTRACTED)
- [happy] named imports from react-native-paper are grouped and sorted deterministically  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `namespace import generation for react imports`  (happy; EXTRACTED)
- [happy] A namespace import 'import * as Dialog from "@radix-ui/react-dialog";' is emitted for namespace-style React imports  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `non-reactive callee hint emission`  (happy; EXTRACTED)
- [happy] A '// dep_inference.over_track' hint comment is emitted when a derived binding calls a non-@reactive in-module function  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `non-route component`  (happy; EXTRACTED)
- [happy] non-route components do not receive screen padding (prevents double-padding)  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `null comparison lowering`  (invariant; EXTRACTED)
- [invariant] Vox 'is null' does not lower to == null or clone() == None comparisons  (crates/vox-codegen/tests/null_and_namespace_lowering.rs)

### `null identifier avoidance`  (invariant; EXTRACTED)
- [invariant] generated Rust does not contain a bare null identifier in statement, terminator, or newline position  (crates/vox-codegen/tests/null_and_namespace_lowering.rs)

### `null literal lowering`  (happy; EXTRACTED)
- [happy] a bare null literal in value position lowers to Rust None  (crates/vox-codegen/tests/null_and_namespace_lowering.rs)

### `numeric binary operators on local variables`  (happy; EXTRACTED)
- [happy] Numeric multiplication and subtraction on local variables emit plain infix without borrow operators (no '& ' appears in output)  (crates/vox-codegen/tests/binary_op_emit.rs)

### `omission of hint when all callees are reactive`  (error; EXTRACTED)
- [error] No dep_inference.over_track hint comment is emitted when all in-module callees are @reactive-annotated  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `over-track hint names the offending callee`  (happy; EXTRACTED)
- [happy] The over-track hint comment names the non-reactive callee (e.g., backtick-quoted 'opaque')  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `pascalize_module_name()`  (happy; EXTRACTED)
- [happy] Converts snake_case and kebab-case module names to PascalCase (counter_store -> CounterStore, user-prefs -> UserPrefs)  (crates/vox-codegen/src/codegen_ts/reactive_module_emit.rs)

### `paths_reference_error_envelope`  (happy; EXTRACTED)
- [happy] generated OpenAPI paths reference ErrorEnvelope schema in responses  (crates/vox-codegen/tests/openapi_crud_api_test.rs)

### `process.run lowering`  (happy; EXTRACTED)
- [happy] Vox process.run call lowers to vox_process_run_opt builtin, not vox_process_run  (crates/vox-codegen/tests/null_and_namespace_lowering.rs)

### `process.run returns Option`  (invariant; EXTRACTED)
- [invariant] process.run lowers to an Option-returning function, not an exit-code-only Result  (crates/vox-codegen/tests/null_and_namespace_lowering.rs)

### `provider setup guidance generation`  (happy; EXTRACTED)
- [happy] Setup guidance text 'requires <MantineProvider>' is emitted for libraries with mandatory provider requirements  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `react import statement generation for external components`  (happy; EXTRACTED)
- [happy] An ES import 'import MyButton from "@acme/btn";' is emitted when an external React component is imported and used in a view  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `reactive function dependency tracking`  (happy; EXTRACTED)
- [happy] A derived binding that calls a @reactive-annotated function includes reactive state in the useMemo dependency array ('useMemo(() => double_it(count), [count])')  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `route validation with loader and pending but no error`  (error; EXTRACTED)
- [error] route with loader and pending but no error component emits validate.route.missing_error error  (crates/vox-codegen/tests/route_completeness_test.rs)

### `route validation with loader but no pending`  (error; EXTRACTED)
- [error] route with loader but no pending component emits validate.route.missing_pending error  (crates/vox-codegen/tests/route_completeness_test.rs)

### `route validation with loader, pending, and error`  (happy; EXTRACTED)
- [happy] route with loader, pending component, and error component passes validation  (crates/vox-codegen/tests/route_completeness_test.rs)

### `route validation without loader`  (happy; EXTRACTED)
- [happy] route without loader does not require pending or error components  (crates/vox-codegen/tests/route_completeness_test.rs)

### `route_file`  (happy; EXTRACTED)
- [happy] route_file generates relative import paths with correct depth (../ nesting) based on the depth parameter  (crates/vox-codegen/src/codegen_ts/rn/routes.rs)

### `row element`  (happy; EXTRACTED)
- [happy] bare row elements emit flexWrap: wrap style to prevent overflow  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `runtime import deduplication across multiple adapters`  (invariant; EXTRACTED)
- [invariant] When both @back_button and @deep_link are present, the @vox/runtime import appears exactly once (deduplicated across both runtime primitives)  (crates/vox-codegen/tests/deep_link_test.rs)

### `scheduled function registration in Tauri main`  (happy; EXTRACTED)
- [happy] When a module contains @scheduled functions, Tauri main.rs calls vox_workflow_runtime::scheduled::register and scheduled::start  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `screen style`  (happy; EXTRACTED)
- [happy] screen style defines horizontal padding of 16  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `screen-root component`  (happy; EXTRACTED)
- [happy] screen-root components wrap their view in a padded screen container  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `script function visibility`  (happy; EXTRACTED)
- [happy] script-defined helper functions are emitted as pub fn in lib.rs so they are visible to the bin via glob import  (crates/vox-codegen/tests/null_and_namespace_lowering.rs)

### `scroll attribute`  (happy; EXTRACTED)
- [happy] row with scroll=horizontal emits a horizontal ScrollView wrapper  (crates/vox-codegen/tests/rn_lifecycle_test.rs)

### `self-import prevention`  (invariant; EXTRACTED)
- [invariant] component's own generated file does not import itself (scoped to component's own .tsx file)  (crates/vox-codegen/tests/component_import_refs_test.rs)

### `sibling component import`  (happy; EXTRACTED)
- [happy] view references to sibling components emit 'import { ComponentName } from "./ComponentName"' statements  (crates/vox-codegen/tests/component_import_refs_test.rs)

### `sibling component reference imports`  (happy; EXTRACTED)
- [happy] A component used as a JSX element (NavBar()) in another component's view emits a relative import from the sibling file 'import { NavBar } from "./NavBar"'  (crates/vox-codegen/tests/component_import_refs_test.rs)

### `sibling import suppression for external components`  (error; EXTRACTED)
- [error] No spurious sibling-relative import './MyButton' is emitted when an external React component already has an ES import  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `sibling import suppression for named components`  (error; EXTRACTED)
- [error] No sibling-relative import is emitted for named external components  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `str + numeric operator codegen rejection`  (error; EXTRACTED)
- [error] String + numeric operands do NOT emit as 's + 5' or 's + &5' (prevents String + i64 type mismatch at runtime)  (crates/vox-codegen/tests/binary_op_emit.rs)

### `str + numeric type coercion codegen`  (happy; EXTRACTED)
- [happy] String concatenated with numeric values emits format!() instead of String + i64, using format!("{}{}") to handle auto-stringify semantics  (crates/vox-codegen/tests/binary_op_emit.rs)

### `tier_z_index`  (invariant; EXTRACTED)
- [invariant] Z-index values for layer tiers form strictly ascending sequence  (crates/vox-codegen/src/web_ir/layer_emit.rs)

### `ts_default_value function for array type`  (happy; EXTRACTED)
- [happy] ts_default_value returns {[]} for array types  (crates/vox-codegen/src/codegen_ts/component.rs)

### `ts_default_value function for boolean type`  (happy; EXTRACTED)
- [happy] ts_default_value returns {false} for boolean types  (crates/vox-codegen/src/codegen_ts/component.rs)

### `ts_default_value function for interface type`  (happy; EXTRACTED)
- [happy] ts_default_value returns {undefined as any} for interface/object types  (crates/vox-codegen/src/codegen_ts/component.rs)

### `ts_default_value function for number type`  (happy; EXTRACTED)
- [happy] ts_default_value returns {0} for number types  (crates/vox-codegen/src/codegen_ts/component.rs)

### `ts_default_value function for optional type`  (happy; EXTRACTED)
- [happy] ts_default_value returns {undefined} for union types with undefined  (crates/vox-codegen/src/codegen_ts/component.rs)

### `ts_default_value function for string type`  (happy; EXTRACTED)
- [happy] ts_default_value returns {\"\"} for string types  (crates/vox-codegen/src/codegen_ts/component.rs)

### `ts_default_value whitespace trimming`  (happy; EXTRACTED)
- [happy] ts_default_value trims leading/trailing whitespace before matching type patterns  (crates/vox-codegen/src/codegen_ts/component.rs)

### `unstyled library css suppression`  (error; EXTRACTED)
- [error] No CSS import is emitted for headless/unstyled libraries like @radix-ui  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `unstyled library provider guidance suppression`  (error; EXTRACTED)
- [error] No provider setup guidance is emitted for headless libraries like @radix-ui  (crates/vox-codegen/src/codegen_ts/reactive.rs)

### `validate_a11y function with img element`  (happy; EXTRACTED)
- [happy] img element with aria-hidden="true" attribute produces no diagnostic errors  (crates/vox-codegen/src/web_ir/validate_a11y.rs)

### `validate_contract_branch`  (happy; EXTRACTED)
- [happy] When validating a route branch with children, validation recursively checks all nested routes and collects all undefined component errors  (crates/vox-codegen/src/codegen_ts/route_manifest.rs)

### `validate_db_projection_suffixes_unique`  (error; EXTRACTED)
- [error] validate_db_projection_suffixes_unique returns an error when select projections have colliding suffixes  (crates/vox-codegen/src/codegen_rust/mod.rs)

### `vox-client Tauri branching`  (happy; EXTRACTED)
- [happy] emit_vox_client() includes Tauri platform branching via isTauri() check  (crates/vox-codegen/tests/tauri_endpoint_client_parity_test.rs)

### `vox-client command invocation`  (happy; EXTRACTED)
- [happy] vox-client invokes command name matching Rust command definition via $tauri  (crates/vox-codegen/tests/tauri_endpoint_client_parity_test.rs)

### `vox-client import generation`  (happy; EXTRACTED)
- [happy] on-mount calls to @query/@mutation endpoints emit 'import { ... } from "./vox-client"' to resolve endpoint references  (crates/vox-codegen/tests/component_import_refs_test.rs)

### `vox_json_parse`  (happy; EXTRACTED)
- [happy] json.parse lowers to the vox_json_parse runtime builtin  (crates/vox-codegen/tests/null_and_namespace_lowering.rs)

### `workflow codegen Phase 1 runtime symbol calls`  (happy; EXTRACTED)
- [happy] Emitted Rust for a workflow function contains calls to ::vox_workflow_runtime::workflow::current_hir_module, interpret_workflow_durable, and extract_terminal_return to execute durable workflows  (crates/vox-codegen/tests/durability_compiles.rs)

### `workflow lowering transformation`  (happy; EXTRACTED)
- [happy] workflow functions lower to calls to interpret_workflow_durable in emitted Rust code  (crates/vox-codegen/tests/durability_lowering.rs)

### `workflow runtime symbol resolution`  (happy; EXTRACTED)
- [happy] workflow emit references ::vox_workflow_runtime::workflow::current_hir_module, ::vox_workflow_runtime::workflow::interpret_workflow_durable, and ::vox_workflow_runtime::workflow::extract_terminal_return in emitted Rust  (crates/vox-codegen/tests/durability_compiles.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`@back_button declaration code generation`** — only: _TypeScript emit includes voxRuntime.onBackButton() call and @vox/runtime adapter import for @back_button declaration_
- **`@query endpoint import in on-mount handlers`** — only: _A @query function called within an on-mount handler emits an import declaration 'import { fn_name } from "./vox-client"'_
- **`AssetManifest icon file validation`** — only: _AssetManifest.validate_preflight() succeeds and stage_under() copies icon files to staging directory_
- **`Backdrop()`** — only: _Backdrop class values can be passed through raw_class attribute without prefix transformation_
- **`BuiltinRegistry inlining for std.time.now_ms`** — only: _BuiltinRegistry.standard().lookup_function("std.time.now_ms", 0) returns BuiltinLowering::Inline("Date.now()") for direct JS expansion_
- **`BuiltinRegistry lookup for str.length`** — only: _BuiltinRegistry.standard().lookup_method("str", "length", 0) returns BuiltinLowering::Property("length") variant_
- **`BuiltinRegistry namespace preservation`** — only: _BuiltinRegistry.standard().lookup_namespace("Speech") returns the canonical name "Speech" (not remapped to alternate like 'mobile')_
- **`CodegenOptions strict_ai=false`** — only: _TypeScript emit records missing-ts-ai-lowering diagnostic when AI fixture present with strict_ai disabled_
- **`Codex database management in tables-only Tauri apps`** — only: _Tables-only Tauri app.manage() the Codex database in generated code_
- **`CounterProvider`** — only: _Generated provider component uses PascalCase naming (CounterProvider.tsx for counter module)_
- **`ErrorEnvelope schema`** — only: _ErrorEnvelope schema contains an ok property with const value false_
- **`File-based speech transcribe without microphone capability`** — only: _Speech.transcribe(file_path) derives speech capability but not microphone in capability_ids_
- **`HirModule embedding in Tauri main`** — only: _Generates src-tauri/src/main.rs calling load_hir_module_from_embedded to embed and register HirModule_
- **`Library mode codegen`** — only: _Library mode generates a package.json with exports for openapi.json_
- **`Link element import`** — only: _link element import from expo-router is included in RN component output_
- **`MyButton component render`** — only: _imported external React component renders as component tag in output_
- **`Panel()`** — only: _Panel() primitive supports background attribute resolution with token values_
- **`PaperProvider guidance emission`** — only: _react-native-paper import triggers provider guidance emission in output_
- **`RoutePattern parsing`** — only: _Literal path segments like /users/me parse into a vector of Segment::Literal variants with correct names_
- **`RoutePattern::overlap_with`** — only: _Returns Overlap::Shadowed when wildcard pattern /files/* is compared with a more-specific literal pattern /files/readme.md_
- **`RustAppShell::AxumLocalServer code generation`** — only: _Generates src/main.rs containing a rust_app_shell marker with value AxumLocalServer_
- **`RustAppShell::AxumLocalServer code generation with AI fixture`** — only: _Generated Rust crate bundle from Vox code passes cargo check successfully_
- **`RustAppShell::TauriApp code generation`** — only: _Generates src-tauri/src/main.rs containing a rust_app_shell marker with value TauriApp_
- **`STT plugin emission when Speech namespace is used`** — only: _Modules using Speech.transcribe_microphone() emit vox_tauri_stt::plugin::init() in Tauri main.rs_
- **`STT plugin omission when Speech is not used`** — only: _Empty module does not emit vox_tauri_stt in Tauri main.rs, build.rs, capabilities/default.json, or Cargo.toml_
- **`Scheduler omission for tables-only modules`** — only: _Tables-only Tauri app does not register a scheduler (no scheduled::register call)_
- **`ScrollView import`** — only: _ScrollView is imported from React Native when row scroll attributes are used_
- **`Tauri InlinedPlugin ACL generation when speech is used`** — only: _Speech usage in Tauri generates src-tauri/build.rs with InlinedPlugin::new() and vox-stt plugin ID_
- **`Tauri api_client_ts emptiness`** — only: _Tauri app generation leaves api_client_ts empty; vox-client.ts is SSOT_
- **`Tauri capability manifest generation`** — only: _Generates src-tauri/capabilities/default.json with vox-stt:default permission when speech is used_
- **`Tauri command attribute`** — only: _#[tauri::command] attribute appears in generated Tauri main.rs_
- **`Tauri command function generation`** — only: _@query function lowers to async fn with tauri::command attribute in main.rs_
- **`Tauri crate excludes web server dependencies`** — only: _Tauri src-tauri/Cargo.toml does not contain axum or rust-embed dependencies_
- **`Tauri plugin dependency with correct relative path`** — only: _Cargo.toml under src-tauri uses ../../../crates/ path for plugin dependencies (vox-tauri-stt)_
- **`Tauri setup block generation for tables-only modules`** — only: _A module with only @table declarations emits a .setup() block in Tauri main.rs_
- **`Tauri shell generation`** — only: _generate() produces RustAppShell::TauriApp with src-tauri/src/main.rs and src-tauri/build.rs files_
- **`WebIR view roots for endpoint-only module`** — only: _endpoint-only module produces empty WebIR view_roots_
- **`WebIrModule`** — only: _validate_web_ir returns empty diagnostic list for default WebIrModule_
- **`Workspace Cargo.toml excludes web server when using Tauri`** — only: _Tauri workspace Cargo.toml root does not list axum dependency_
- **`activity lowering transformation`** — only: _activity functions lower to journal::execute wrapper and reference activity_id in emitted Rust code_
- **`activity runtime symbol resolution`** — only: _activity emit references ::vox_workflow_runtime::journal::execute in emitted Rust_
- **`actor dispatch routing`** — only: _actor shell emits real Envelope dispatch table with message/request routing, handler names as string literals, and ProcessContext::reply calls, removing no-op dispatch markers_
- **`actor lowering transformation`** — only: _actor declarations lower to vox_actor_runtime::spawn_process calls in emitted Rust code_
- **`async IIFE`** — only: _awaited async endpoint calls run in an async IIFE_
- **`async endpoint call`** — only: _async endpoint calls in on-mount handlers are awaited_
- **`bare_package extraction for scoped subpaths`** — only: _bare_package(@mui/material/Button) returns Some(@mui/material)_
- **`bare_package for relative paths`** — only: _bare_package returns None for relative paths like ./Foo.tsx or ../ui/Bar_
- **`bare_package for unscoped subpaths`** — only: _bare_package(react-aria/useButton) returns Some(react-aria)_
- **`bleed attribute`** — only: _screen roots with bleed=true opt out of default screen padding_
- **`class_string()`** — only: _class_string() method on resolved stack primitive joins CSS classes with space, containing 'flex' and 'flex-col'_
- **`collect_table_select_projections`** — only: _collects SELECT projections from HirDbQueryPlan within endpoint function bodies_
- **`css injection for styled libraries`** — only: _An auto-injected CSS import 'import "@mantine/core/styles.css";' is emitted when a CSS-dependent library (@mantine/core) is imported_
- **`deep-link runtime adapter emission`** — only: _deep-link configuration emits voxRuntime.onDeepLink(), handler call, useDeepLinkRouting hook, useEffect import, and @vox/runtime adapter import_
- **`deep_link handler runtime adapter integration`** — only: _A @deep_link declaration emits calls to the vox runtime adapter (voxRuntime.onDeepLink) and imports the useDeepLinkRouting hook from @vox/runtime_
- **`deep_link useEffect import`** — only: _A @deep_link declaration results in an import of useEffect from React for lifecycle integration_
- **`direct argument reads included in dep array`** — only: _When a derived binding directly references state in the argument passed to a function, that state is included in the dep array even if the function is non-@reactive_
- **`discarded endpoint call`** — only: _discarded endpoint calls do not emit dead const _ = promise bindings_
- **`emit_app_hooks_default`** — only: _Default app hooks imports ErrorBoundary from vox-error-boundary module_
- **`emit_async_view_tsx`** — only: _Emitted async view discriminates on _tag field for all four async states_
- **`emit_cargo_toml`** — only: _emit_cargo_toml includes rust_imports (crate names and versions) in the generated full-app Cargo.toml_
- **`emit_dialog_primitive`** — only: _emit_dialog_primitive generates TSX containing role=dialog, aria-modal=true, aria-label binding, Tab key handler, focus restoration, and Escape handler_
- **`emit_fn for numeric addition`** — only: _emit_fn emits clean `1 + 2` without spurious RHS borrow (`1 + &2`)_
- **`emit_fn with @ai task_category and strengths attributes`** — only: _emit_fn maps task_category and strengths attributes to LlmConfig telemetry fields_
- **`emit_fn with @ai(structured_output=...)`** — only: _emit_fn wires response_format through LlmConfig when structured_output is declared, including schema name_
- **`emit_fn with @prompt fixture`** — only: _emit_fn emits cascade_for_research_stage construction with matching ResearchStage when @prompt fixture present_
- **`emit_fn with @search fixture`** — only: _emit_fn references lookup_fact_by_key and emits SearchDispatch telemetry for @search fixture_
- **`emit_fn with @subagent(policy=distributed)`** — only: _emit_fn calls relay_ai_fixture_distributed_subagent and gates with populi-transport cfg feature_
- **`emit_fn with @subagent(policy=parallel)`** — only: _emit_fn creates DispatchRouter and calls route_with_telemetry for parallel subagent policy_
- **`emit_fn without structured_output`** — only: _emit_fn omits response_format injection when structured_output is not declared_
- **`emit_hir_expr`** — only: _emits all branches of nested JSX if-expressions as nested ternaries without void IIFEs_
- **`emit_hir_expr_attr_value`** — only: _emits await for async endpoint calls in let-binding assignments within handlers_
- **`emit_ident_expr()`** — only: _A borrowed str identifier emits as .as_str() method call_
- **`emit_main route registration`** — only: _emit_main generates .route() calls with GET for Query endpoints and POST for Mutation endpoints, with matching async handler functions_
- **`emit_main_boot() generates valid JSON`** — only: _emit_main_boot generates a raw string EMBEDDED_HIR containing valid JSON that deserializes to HirModule_
- **`emit_mobile_setup`** — only: _shell_projection_round_trip produces emit_mobile_setup output that matches the snapshot_
- **`emit_openclaw_or_browser_registry_call()`** — only: _Scrape namespace calls emit to vox_scrape_* runtime symbols without wasm32 guard_
- **`emit_runtime_install`** — only: _Runtime installation exposes global functions and test transcript hook_
- **`emit_sitemap_xml`** — only: _Emitted sitemap XML includes loc entries for all routes_
- **`emit_state_machine_decls`** — only: _Returns empty string when module contains no state machine declarations_
- **`emit_sw_register`** — only: _Service worker registration exports async function registerServiceWorker_
- **`emit_table_struct`** — only: _emit_table_struct emits a field pub _id: Option<i64> for table structures_
- **`emit_table_struct Option field handling`** — only: _emit_table_struct generates from_row code that deserializes Option<int> fields using row.get::<Option<i64>>()_
- **`emit_table_struct SQL generation`** — only: _emit_table_struct generates SQL FROM, INSERT INTO, and DELETE FROM clauses using lowercase table names_
- **`emit_table_struct bool field handling`** — only: _emit_table_struct generates from_row code that deserializes boolean fields using row.get::<i64>() with != 0 check_
- **`emit_table_struct find method`** — only: _emit_table_struct generates find method returning Result<Self, turso::Error> to enforce non-null policy and error when records are missing_
- **`emit_table_struct projection support`** — only: _emit_table_struct emits from_row_sel_* and all_proj_* methods with explicit column lists in SELECT SQL when projections are configured_
- **`emit_tabs_primitive`** — only: _emit_tabs_primitive generates TSX containing role=tablist, aria-selected attribute, role=tab, role=tabpanel, and aria-labelledby_
- **`emit_tokens_ts`** — only: _TypeScript output exports voxTokens const with 'as const' and VoxTokenKey type_
- **`emit_web_entry`** — only: _Generated web entry imports runtime-install module_
- **`emit_with()`** — only: _With emission produces execute_activity_result() call without panic mapping, preserving stable activity identity_
- **`endpoint call transformation`** — only: _positional multi-arg endpoint calls are rewritten to named-object form matching vox-client signature, and positional form is eliminated_
- **`endpoint positional-to-named argument rewriting`** — only: _Multi-argument endpoint function calls (e.g., record_event("mood", "{}")) are rewritten to named-argument object form (record_event({ kind: "mood", payload: "{}" })) to match vox-client.ts endpoint function signature_
- **`error arm lowering`** — only: _Result-returning builtins emit Err(m) for error arms, not the undefined Error(m) constructor_
- **`error envelope code field`** — only: _error_envelope.example.json has non-empty code string field_
- **`error envelope details field`** — only: _error_envelope.example.json has details field present_
- **`error envelope message field`** — only: _error_envelope.example.json has non-empty message string field_
- **`error envelope ok field`** — only: _error_envelope.example.json has ok field set to false_
- **`error envelope request_id field`** — only: _error_envelope.example.json has request_id field present_
- **`expo_router_fs_path`** — only: _expo_router_fs_path converts path parameter notation (e.g., :id) to bracket notation (e.g., [id])_
- **`external React import`** — only: _external React component import emits ES6 import statement not sibling import_
- **`external react component tag rendering`** — only: _An external React component is rendered as a JSX component tag (<MyButton) in the emitted code_
- **`extract_state_deps`** — only: _extracts state identifiers from binary expression operands_
- **`extract_state_deps_with_callees`** — only: _recursively descends into reactive callee bodies to find state dependencies not visible at call site_
- **`fire-and-forget endpoint`** — only: _fire-and-forget endpoint calls are wrapped with .catch() to prevent unhandled promise rejections_
- **`generate`** — only: _Form emission exports a React component with the form name_
- **`generate_script`** — only: _generate_script emits a Cargo.toml with tracing dependency when log.* builtins are lowered_
- **`generate_script_with_target Cargo.toml generation`** — only: _generate_script_with_target includes rust_imports (crate names and versions) in the generated Cargo.toml for script targets_
- **`generate_script_with_target for Wasi`** — only: _generate_script_with_target succeeds (returns Ok) for empty modules compiled to Wasi target_
- **`grouped named imports for external components`** — only: _Named imports are emitted as a grouped import statement 'import { Dialog, DialogContent } from "@radix-ui/react-dialog";'_
- **`import statement generation for cross-component usage`** — only: _When a component Inner is referenced within another component Outer's view, an ES import statement 'import { Inner } from "./Inner";' is emitted in the Outer.tsx output_
- **`import()`** — only: _Badge component import is emitted and detected inside conditional branch (if/else) in generated code_
- **`is null lowering`** — only: _Vox 'is null' operator on an Option type lowers to Rust .is_none() method call_
- **`isnt null lowering`** — only: _Vox 'isnt null' operator on an Option type lowers to Rust .is_some() method call_
- **`key() emission`** — only: _emitting TypeScript for a list render with a field key produces output containing 'key={w}' or 'key='_
- **`link href attribute conversion`** — only: _link(href=...) source converts to <Link href={{...}}> JSX in output_
- **`link label wrapping in Text element`** — only: _link label content is wrapped in <Text> element for RN compatibility_
- **`lookup resolves @mantine/core styling`** — only: _lookup(@mantine/core) returns library with Styling::CssFile_
- **`lookup resolves @mui/material subpath to bare package`** — only: _lookup(@mui/material/Button) returns library with package @mui/material_
- **`lookup returns None for relative paths`** — only: _lookup(./local) returns None for relative paths_
- **`lookup returns None for unknown packages`** — only: _lookup(totally-unknown-pkg) returns None_
- **`named component tag rendering`** — only: _Named external React components are rendered as JSX component tags (<Dialog, <DialogContent)_
- **`named import grouping`** — only: _named imports from react-native-paper are grouped and sorted deterministically_
- **`namespace import generation for react imports`** — only: _A namespace import 'import * as Dialog from "@radix-ui/react-dialog";' is emitted for namespace-style React imports_
- **`non-reactive callee hint emission`** — only: _A '// dep_inference.over_track' hint comment is emitted when a derived binding calls a non-@reactive in-module function_
- **`non-route component`** — only: _non-route components do not receive screen padding (prevents double-padding)_
- **`null literal lowering`** — only: _a bare null literal in value position lowers to Rust None_
- **`numeric binary operators on local variables`** — only: _Numeric multiplication and subtraction on local variables emit plain infix without borrow operators (no '& ' appears in output)_
- **`over-track hint names the offending callee`** — only: _The over-track hint comment names the non-reactive callee (e.g., backtick-quoted 'opaque')_
- **`pascalize_module_name()`** — only: _Converts snake_case and kebab-case module names to PascalCase (counter_store -> CounterStore, user-prefs -> UserPrefs)_
- **`paths_reference_error_envelope`** — only: _generated OpenAPI paths reference ErrorEnvelope schema in responses_
- **`process.run lowering`** — only: _Vox process.run call lowers to vox_process_run_opt builtin, not vox_process_run_
- **`provider setup guidance generation`** — only: _Setup guidance text 'requires <MantineProvider>' is emitted for libraries with mandatory provider requirements_
- **`react import statement generation for external components`** — only: _An ES import 'import MyButton from "@acme/btn";' is emitted when an external React component is imported and used in a view_
- **`reactive function dependency tracking`** — only: _A derived binding that calls a @reactive-annotated function includes reactive state in the useMemo dependency array ('useMemo(() => double_it(count), [count])')_
- **`resolve`** — only: _resolve("stack") returns an element with html_tag "div" and base_classes containing both "flex" and "flex-col"_
- **`route validation with loader, pending, and error`** — only: _route with loader, pending component, and error component passes validation_
- **`route validation without loader`** — only: _route without loader does not require pending or error components_
- **`route_file`** — only: _route_file generates relative import paths with correct depth (../ nesting) based on the depth parameter_
- **`row element`** — only: _bare row elements emit flexWrap: wrap style to prevent overflow_
- **`scheduled function registration in Tauri main`** — only: _When a module contains @scheduled functions, Tauri main.rs calls vox_workflow_runtime::scheduled::register and scheduled::start_
- **`screen style`** — only: _screen style defines horizontal padding of 16_
- **`screen-root component`** — only: _screen-root components wrap their view in a padded screen container_
- **`script function visibility`** — only: _script-defined helper functions are emitted as pub fn in lib.rs so they are visible to the bin via glob import_
- **`scroll attribute`** — only: _row with scroll=horizontal emits a horizontal ScrollView wrapper_
- **`sibling component import`** — only: _view references to sibling components emit 'import { ComponentName } from "./ComponentName"' statements_
- **`sibling component reference imports`** — only: _A component used as a JSX element (NavBar()) in another component's view emits a relative import from the sibling file 'import { NavBar } from "./NavBar"'_
- **`str + numeric type coercion codegen`** — only: _String concatenated with numeric values emits format!() instead of String + i64, using format!("{}{}") to handle auto-stringify semantics_
- **`text()`** — only: _text() primitive supports color token resolution with dash-separated syntax_
- **`ts_default_value function for array type`** — only: _ts_default_value returns {[]} for array types_
- **`ts_default_value function for boolean type`** — only: _ts_default_value returns {false} for boolean types_
- **`ts_default_value function for interface type`** — only: _ts_default_value returns {undefined as any} for interface/object types_
- **`ts_default_value function for number type`** — only: _ts_default_value returns {0} for number types_
- **`ts_default_value function for optional type`** — only: _ts_default_value returns {undefined} for union types with undefined_
- **`ts_default_value function for string type`** — only: _ts_default_value returns {\"\"} for string types_
- **`ts_default_value whitespace trimming`** — only: _ts_default_value trims leading/trailing whitespace before matching type patterns_
- **`valid_for_target`** — only: _valid_for_target returns true for @mantine/core on web target and false on react-native target_
- **`validate_a11y function with img element`** — only: _img element with aria-hidden="true" attribute produces no diagnostic errors_
- **`validate_contract_branch`** — only: _When validating a route branch with children, validation recursively checks all nested routes and collects all undefined component errors_
- **`vox-client Tauri branching`** — only: _emit_vox_client() includes Tauri platform branching via isTauri() check_
- **`vox-client command invocation`** — only: _vox-client invokes command name matching Rust command definition via $tauri_
- **`vox-client import generation`** — only: _on-mount calls to @query/@mutation endpoints emit 'import { ... } from "./vox-client"' to resolve endpoint references_
- **`vox_json_parse`** — only: _json.parse lowers to the vox_json_parse runtime builtin_
- **`workflow codegen Phase 1 runtime symbol calls`** — only: _Emitted Rust for a workflow function contains calls to ::vox_workflow_runtime::workflow::current_hir_module, interpret_workflow_durable, and extract_terminal_return to execute durable workflows_
- **`workflow lowering transformation`** — only: _workflow functions lower to calls to interpret_workflow_durable in emitted Rust code_
- **`workflow runtime symbol resolution`** — only: _workflow emit references ::vox_workflow_runtime::workflow::current_hir_module, ::vox_workflow_runtime::workflow::interpret_workflow_durable, and ::vox_workflow_runtime::workflow::extract_terminal_return in emitted Rust_
