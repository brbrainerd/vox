use crate::ast::decl::*;
use crate::ast::expr;
use crate::ast::types::TypeExpr;
use crate::hir::nodes::form::{HirFieldConstraint, HirForm, HirFormField};
use crate::hir::*;

use super::LowerCtx;

impl LowerCtx {
    pub(crate) fn lower_fn(&mut self, f: &FnDecl) -> HirFn {
        let id = self.def_map.define(f.name.clone());
        self.def_map.push_scope();
        let params = f.params.iter().map(|p| self.lower_param(p)).collect();
        let mut body = f.body.iter().map(|s| self.lower_stmt(s)).collect();
        body = self.inject_contracts(f, body);
        self.def_map.pop_scope();

        let capabilities = Self::merge_mens_capabilities(
            f.effects
                .iter()
                .map(|e| match e {
                    crate::ast::decl::EffectAnnotation::Net => crate::hir::HirCapability::Net,
                    crate::ast::decl::EffectAnnotation::Db => crate::hir::HirCapability::Db,
                    crate::ast::decl::EffectAnnotation::Fs => crate::hir::HirCapability::Fs,
                    crate::ast::decl::EffectAnnotation::Env => crate::hir::HirCapability::Env,
                    crate::ast::decl::EffectAnnotation::Clock => crate::hir::HirCapability::Clock,
                    crate::ast::decl::EffectAnnotation::Random => crate::hir::HirCapability::Random,
                    crate::ast::decl::EffectAnnotation::Spawn => crate::hir::HirCapability::Spawn,
                    crate::ast::decl::EffectAnnotation::GpuCompute => {
                        crate::hir::HirCapability::GpuCompute
                    }
                    crate::ast::decl::EffectAnnotation::Mutate => crate::hir::HirCapability::Mutate,
                    crate::ast::decl::EffectAnnotation::Mcp(t) => {
                        crate::hir::HirCapability::Mcp(t.clone())
                    }
                    crate::ast::decl::EffectAnnotation::Nothing => {
                        crate::hir::HirCapability::Nothing
                    }
                })
                .collect(),
            f.inference_model.is_some(),
            f.training_step,
        );

        HirFn {
            id,
            name: f.name.clone(),
            generics: f.generics.clone(),
            params,
            return_type: f.return_type.as_ref().map(|t| self.lower_type(t)),
            body,
            is_async: false,
            is_pub: f.is_pub,
            is_mobile_native: f.is_mobile_native,
            is_pure: f.is_pure,
            is_reactive: f.is_reactive,
            is_remote: f.is_remote,
            is_llm: f.is_llm,
            llm_model: f.llm_model.clone(),
            ai_structured_output: f.ai_structured_output_type.as_ref().map(|ty| {
                crate::hir::nodes::boilerplate_grafts::HirAiStructuredOutput {
                    return_type: ty.clone(),
                    max_iterations: if f.ai_max_iterations == 0 {
                        3
                    } else {
                        f.ai_max_iterations
                    },
                    span: f.span,
                }
            }),
            ai_fixture: if f.hole_spec.is_some() || f.hole_cache_key.is_some() {
                Some(crate::hir::nodes::boilerplate_grafts::HirAiFixture::Hole(
                    crate::hir::nodes::boilerplate_grafts::HirHoleFixture {
                        spec: f
                            .hole_spec
                            .clone()
                            .unwrap_or_else(|| "unfilled-hole".to_string()),
                        reviewer: f
                            .hole_reviewer
                            .clone()
                            .unwrap_or_else(|| "human".to_string()),
                        cache_key: f
                            .hole_cache_key
                            .clone()
                            .unwrap_or_else(|| "unset".to_string()),
                        constraints: f.hole_constraints.clone(),
                        span: f.span,
                    },
                ))
            } else if f.is_llm {
                let has_prompt_payload = f.prompt_stage.is_some() && f.prompt_schema.is_some();
                let has_subagent_payload = f.subagent_policy.is_some() || f.subagent_max_depth.is_some();
                let has_search_payload = f.search_corpus.is_some()
                    || f.search_query.is_some()
                    || f.search_into.is_some()
                    || f.search_top_k.is_some()
                    || f.search_policy.is_some();
                let has_intent_payload = f.ai_task_category.is_some()
                    || !f.ai_strengths.is_empty()
                    || f.ai_tier_max.is_some()
                    || f.ai_cost_ceiling_usd_per_call.is_some();
                if has_search_payload {
                    Some(crate::hir::nodes::boilerplate_grafts::HirAiFixture::Search(
                        crate::hir::nodes::boilerplate_grafts::HirSearchFixture {
                            corpus: f
                                .search_corpus
                                .clone()
                                .unwrap_or_else(|| "docs".to_string()),
                            query: f
                                .search_query
                                .clone()
                                .unwrap_or_else(|| String::from("")),
                            into_type: f
                                .search_into
                                .clone()
                                .or_else(|| {
                                    f.return_type.as_ref().and_then(|t| match t {
                                        TypeExpr::Named { name, .. } => Some(name.clone()),
                                        _ => None,
                                    })
                                })
                                .unwrap_or_else(|| "str".to_string()),
                            top_k: f.search_top_k,
                            policy: f.search_policy.clone(),
                            span: f.span,
                        },
                    ))
                } else if has_subagent_payload {
                    Some(
                        crate::hir::nodes::boilerplate_grafts::HirAiFixture::Subagent(
                            crate::hir::nodes::boilerplate_grafts::HirSubagentFixture {
                                policy: f
                                    .subagent_policy
                                    .clone()
                                    .unwrap_or_else(|| "inline_only".to_string()),
                                max_depth: f.subagent_max_depth.unwrap_or(1),
                                budget_usd: f.subagent_budget_usd,
                                description: f.subagent_description.clone(),
                                parallel: f.subagent_parallel,
                                complexity: f.subagent_complexity,
                                span: f.span,
                            },
                        ),
                    )
                } else if has_prompt_payload {
                    Some(crate::hir::nodes::boilerplate_grafts::HirAiFixture::Prompt(
                        crate::hir::nodes::boilerplate_grafts::HirPromptFixture {
                            stage: f.prompt_stage.clone().unwrap_or_else(|| "Planner".to_string()),
                            schema: f.prompt_schema.clone().unwrap_or_else(|| "Unknown".to_string()),
                            redact: f.prompt_redact.clone(),
                            span: f.span,
                        },
                    ))
                } else if has_intent_payload {
                    Some(crate::hir::nodes::boilerplate_grafts::HirAiFixture::IntentRouted(
                        crate::hir::nodes::boilerplate_grafts::HirAiIntentFixture {
                            task_category: f.ai_task_category.clone(),
                            strengths: f.ai_strengths.clone(),
                            tier_max: f.ai_tier_max.clone(),
                            cost_ceiling_usd_per_call: f.ai_cost_ceiling_usd_per_call,
                            span: f.span,
                        },
                    ))
                } else {
                    Some(crate::hir::nodes::boilerplate_grafts::HirAiFixture::ModelPin(
                        crate::hir::nodes::boilerplate_grafts::HirAiModelPinFixture {
                            model: f.llm_model.clone(),
                            structured_output: f.ai_structured_output_type.as_ref().map(|ty| {
                                crate::hir::nodes::boilerplate_grafts::HirAiStructuredOutput {
                                    return_type: ty.clone(),
                                    max_iterations: if f.ai_max_iterations == 0 {
                                        3
                                    } else {
                                        f.ai_max_iterations
                                    },
                                    span: f.span,
                                }
                            }),
                            span: f.span,
                        },
                    ))
                }
            } else {
                None
            },
            embed: f
                .embed
                .as_ref()
                .map(|e| crate::hir::nodes::boilerplate_grafts::HirEmbedDecl {
                    model: e.model.clone(),
                    source_field: e.source_field.clone(),
                    dimension: e.dimensions,
                    span: e.span,
                }),
            is_deprecated: f.is_deprecated,
            schedule_interval: None,
            durability: None,
            actor_state_fields: vec![],
            capabilities,
            postconditions: f
                .postconditions
                .iter()
                .map(|p| HirPostCondition {
                    condition: self.lower_expr(&p.condition),
                    fallback: p.fallback.clone(),
                })
                .collect(),
            ts_extern_module: f.ts_extern_module.clone(),
            generated_hash: None,
            span: f.span,
            inference_model: f.inference_model.clone(),
            training_step: f.training_step,
            distributed_train: None,
        }
    }

    fn merge_mens_capabilities(
        mut caps: Vec<crate::hir::HirCapability>,
        inference: bool,
        training_step: bool,
    ) -> Vec<crate::hir::HirCapability> {
        let mut push = |c: crate::hir::HirCapability| {
            if !caps.contains(&c) {
                caps.push(c);
            }
        };
        if inference {
            push(crate::hir::HirCapability::GpuCompute);
            push(crate::hir::HirCapability::Random);
            push(crate::hir::HirCapability::Net);
        }
        if training_step {
            push(crate::hir::HirCapability::GpuCompute);
            push(crate::hir::HirCapability::Random);
            push(crate::hir::HirCapability::Mutate);
        }
        caps
    }

    pub(crate) fn lower_param(&mut self, p: &expr::Param) -> HirParam {
        let id = self.def_map.define(p.name.clone());
        HirParam {
            id,
            name: p.name.clone(),
            type_ann: p.type_ann.as_ref().map(|t| self.lower_type(t)),
            default: p.default.as_ref().map(|e| self.lower_expr(e)),
            span: p.span,
        }
    }

    pub(crate) fn lower_type(&self, t: &TypeExpr) -> HirType {
        match t {
            TypeExpr::Named { name, .. } => {
                if name == "Unit" {
                    HirType::Unit
                } else {
                    HirType::Named(name.clone())
                }
            }
            TypeExpr::Generic { name, args, .. } => HirType::Generic(
                name.clone(),
                args.iter().map(|a| self.lower_type(a)).collect(),
            ),
            TypeExpr::Function {
                params,
                return_type,
                ..
            } => HirType::Function(
                params.iter().map(|p| self.lower_type(p)).collect(),
                Box::new(self.lower_type(return_type)),
            ),
            TypeExpr::Tuple { elements, .. } => {
                HirType::Tuple(elements.iter().map(|e| self.lower_type(e)).collect())
            }
            TypeExpr::Unit { .. } => HirType::Unit,
            TypeExpr::Infer { .. } => HirType::Named("_".to_string()),
            TypeExpr::Decimal { .. } => HirType::Decimal,
        }
    }
    pub(crate) fn lower_typedef(&mut self, t: &TypeDefDecl) -> HirTypeDef {
        let id = self.def_map.define(t.name.clone());
        HirTypeDef {
            id,
            name: t.name.clone(),
            variants: t
                .variants
                .iter()
                .map(|v| HirVariant {
                    name: v.name.clone(),
                    fields: v
                        .fields
                        .iter()
                        .map(|f| (f.name.clone(), self.lower_type(&f.type_ann)))
                        .collect(),
                    span: v.span,
                })
                .collect(),
            fields: t
                .fields
                .iter()
                .map(|f| (f.name.clone(), self.lower_type(&f.type_ann)))
                .collect(),
            is_pub: t.is_pub,
            span: t.span,
        }
    }

    pub(crate) fn lower_table(&mut self, t: &TableDecl) -> HirTable {
        let id = self.def_map.define(t.name.clone());
        HirTable {
            id,
            name: t.name.clone(),
            fields: t
                .fields
                .iter()
                .map(|f| HirTableField {
                    name: f.name.clone(),
                    type_ann: self.lower_type(&f.type_ann),
                    span: f.span,
                })
                .collect(),
            is_pub: t.is_pub,
            is_deprecated: t.is_deprecated,
            span: t.span,
        }
    }

    pub(crate) fn lower_collection(&mut self, c: &CollectionDecl) -> HirCollection {
        let id = self.def_map.define(c.name.clone());
        HirCollection {
            id,
            name: c.name.clone(),
            fields: c
                .fields
                .iter()
                .map(|f| HirTableField {
                    name: f.name.clone(),
                    type_ann: self.lower_type(&f.type_ann),
                    span: f.span,
                })
                .collect(),
            is_pub: c.is_pub,
            has_spread: c.has_spread,
            span: c.span,
        }
    }

    pub(crate) fn lower_reactive_component(
        &mut self,
        r: &ReactiveComponentDecl,
    ) -> HirReactiveComponent {
        let id = self.def_map.define(r.name.clone());
        self.def_map.push_scope();
        let params = r.params.iter().map(|p| self.lower_param(p)).collect();
        let members: Vec<HirReactiveMember> = r
            .members
            .iter()
            .map(|m| match m {
                ReactiveMemberDecl::State(s) => HirReactiveMember::State(HirState {
                    id: self.def_map.define(s.name.clone()),
                    name: s.name.clone(),
                    ty: s.ty.as_ref().map(|t| self.lower_type(t)),
                    init: self.lower_expr(&s.init),
                    span: s.span,
                }),
                ReactiveMemberDecl::Derived(d) => HirReactiveMember::Derived(HirDerived {
                    id: self.def_map.define(d.name.clone()),
                    name: d.name.clone(),
                    ty: d.ty.as_ref().map(|t| self.lower_type(t)),
                    expr: self.lower_expr(&d.expr),
                    span: d.span,
                }),
                ReactiveMemberDecl::Effect(e) => HirReactiveMember::Effect(HirEffect {
                    body: self.lower_expr(&e.body),
                    explicit_deps: e.explicit_deps.clone(),
                    span: e.span,
                }),
                ReactiveMemberDecl::OnMount(m) => HirReactiveMember::OnMount(HirOnMount {
                    body: self.lower_expr(&m.body),
                    span: m.span,
                }),
                ReactiveMemberDecl::OnCleanup(c) => HirReactiveMember::OnCleanup(HirOnCleanup {
                    body: self.lower_expr(&c.body),
                    span: c.span,
                }),
                ReactiveMemberDecl::Stmt(s) => HirReactiveMember::Stmt(self.lower_stmt(s)),
            })
            .collect();
        let view = r.view.as_ref().map(|v| self.lower_expr(v));
        self.def_map.pop_scope();

        HirReactiveComponent {
            id,
            name: r.name.clone(),
            params,
            members,
            view,
            styles: r.styles.clone(),
            span: r.span,
        }
    }

    pub(crate) fn lower_agent(&mut self, a: &AgentDecl) -> HirAgent {
        let id = self.def_map.define(a.name.clone());
        HirAgent {
            id,
            name: a.name.clone(),
            version: a.version.clone(),
            state_fields: a
                .state_fields
                .iter()
                .map(|f| HirTableField {
                    name: f.name.clone(),
                    type_ann: self.lower_type(&f.type_ann),
                    span: f.span,
                })
                .collect(),
            handlers: a
                .handlers
                .iter()
                .map(|h| {
                    self.def_map.push_scope();
                    let params = h.params.iter().map(|p| self.lower_param(p)).collect();
                    let body = h.body.iter().map(|s| self.lower_stmt(s)).collect();
                    self.def_map.pop_scope();
                    HirAgentHandler {
                        event_name: h.event_name.clone(),
                        params,
                        return_type: h.return_type.as_ref().map(|t| self.lower_type(t)),
                        body,
                        is_traced: h.is_traced,
                        span: h.span,
                    }
                })
                .collect(),
            migrations: a
                .migrations
                .iter()
                .map(|m| {
                    self.def_map.push_scope();
                    let body = m.body.iter().map(|s| self.lower_stmt(s)).collect();
                    self.def_map.pop_scope();
                    HirMigrationRule {
                        from_version: m.from_version.clone(),
                        body,
                        span: m.span,
                    }
                })
                .collect(),
            is_deprecated: a.is_deprecated,
            span: a.span,
        }
    }

    pub(crate) fn lower_environment(&mut self, e: &EnvironmentDecl) -> HirEnvironment {
        HirEnvironment {
            name: e.name.clone(),
            base_image: e.base_image.clone(),
            packages: e.packages.clone(),
            env_vars: e.env_vars.clone(),
            exposed_ports: e.exposed_ports.clone(),
            volumes: e.volumes.clone(),
            workdir: e.workdir.clone(),
            cmd: e.cmd.clone(),
            copy_instructions: e.copy_instructions.clone(),
            run_commands: e.run_commands.clone(),
            is_deprecated: e.is_deprecated,
            span: e.span,
        }
    }



    pub(crate) fn lower_url_decl(&mut self, u: &UrlDecl) -> HirUrlDecl {
        let id = self.def_map.define(u.name.clone());
        HirUrlDecl {
            id,
            name: u.name.clone(),
            variants: u
                .variants
                .iter()
                .map(|v| HirUrlVariant {
                    name: v.name.clone(),
                    args: v
                        .args
                        .iter()
                        .map(|a| HirUrlArg {
                            name: a.name.clone(),
                            optional: a.optional,
                            ty: self.lower_type(&a.type_ann),
                            span: a.span,
                        })
                        .collect(),
                    span: v.span,
                })
                .collect(),
            is_pub: u.is_pub,
            span: u.span,
        }
    }

    /// Lower a `workflow` declaration to `HirFn` (durability set by caller).
    pub(crate) fn lower_workflow(&mut self, w: &crate::ast::decl::WorkflowDecl) -> HirFn {
        let id = self.def_map.define(w.name.clone());
        self.def_map.push_scope();
        let params = w.params.iter().map(|p| self.lower_param(p)).collect();
        let body = w.body.iter().map(|s| self.lower_stmt(s)).collect();
        self.def_map.pop_scope();
        let distributed_train = match (&w.distributed_train_strategy, &w.distributed_train_peers) {
            (Some(s), Some(p)) => Some((s.clone(), (*p).min(u64::from(u32::MAX)) as u32)),
            _ => None,
        };
        let mut capabilities = vec![];
        if distributed_train.is_some() {
            let mut push = |c: crate::hir::HirCapability| {
                if !capabilities.contains(&c) {
                    capabilities.push(c);
                }
            };
            push(crate::hir::HirCapability::Spawn);
            push(crate::hir::HirCapability::Net);
            push(crate::hir::HirCapability::GpuCompute);
            push(crate::hir::HirCapability::Random);
            push(crate::hir::HirCapability::Mutate);
        }
        HirFn {
            id,
            name: w.name.clone(),
            generics: vec![],
            params,
            return_type: w.return_type.as_ref().map(|t| self.lower_type(t)),
            body,
            is_async: false,
            is_pub: false,
            is_mobile_native: false,
            is_pure: false,
            is_reactive: false,
            is_remote: false,
            is_llm: false,
            llm_model: None,
            ai_structured_output: None,
            ai_fixture: None,
            embed: None,
            is_deprecated: w.is_deprecated,
            schedule_interval: None,
            durability: None, // overwritten by caller
            actor_state_fields: vec![],
            capabilities,
            postconditions: vec![],
            ts_extern_module: None,
            generated_hash: None,
            span: w.span,
            inference_model: None,
            training_step: false,
            distributed_train,
        }
    }

    /// Lower an `activity` declaration to `HirFn` (durability set by caller).
    pub(crate) fn lower_activity(&mut self, a: &crate::ast::decl::ActivityDecl) -> HirFn {
        let id = self.def_map.define(a.name.clone());
        self.def_map.push_scope();
        let params = a.params.iter().map(|p| self.lower_param(p)).collect();
        let body = a.body.iter().map(|s| self.lower_stmt(s)).collect();
        self.def_map.pop_scope();
        HirFn {
            id,
            name: a.name.clone(),
            generics: vec![],
            params,
            return_type: a.return_type.as_ref().map(|t| self.lower_type(t)),
            body,
            is_async: false,
            is_pub: false,
            is_mobile_native: false,
            is_pure: false,
            is_reactive: false,
            is_remote: false,
            is_llm: false,
            llm_model: None,
            ai_structured_output: None,
            ai_fixture: None,
            embed: None,
            is_deprecated: a.is_deprecated,
            schedule_interval: None,
            durability: None, // overwritten by caller
            actor_state_fields: vec![],
            capabilities: vec![],
            postconditions: vec![],
            ts_extern_module: None,
            generated_hash: None,
            span: a.span,
            inference_model: None,
            training_step: false,
            distributed_train: None,
        }
    }

    /// Lower an `actor` declaration to an actor-shell `HirFn` (durability set by caller).
    /// State fields are attached to the shell. Call [`lower_actor_handlers`] to obtain
    /// the per-handler `HirFn` entries that carry the executable bodies.
    pub(crate) fn lower_actor(&mut self, a: &crate::ast::decl::logic::ActorDecl) -> HirFn {
        use crate::hir::HirTableField;
        let id = self.def_map.define(a.name.clone());
        let actor_state_fields = a
            .state_fields
            .iter()
            .map(|f| HirTableField {
                name: f.name.clone(),
                type_ann: self.lower_type(&f.type_ann),
                span: f.span,
            })
            .collect();
        HirFn {
            id,
            name: a.name.clone(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
            is_async: false,
            is_pub: false,
            is_mobile_native: false,
            is_pure: false,
            is_reactive: false,
            is_remote: false,
            is_llm: false,
            llm_model: None,
            ai_structured_output: None,
            ai_fixture: None,
            embed: None,
            is_deprecated: a.is_deprecated,
            schedule_interval: None,
            durability: None, // overwritten by caller
            actor_state_fields,
            capabilities: vec![],
            postconditions: vec![],
            ts_extern_module: None,
            generated_hash: None,
            span: a.span,
            inference_model: None,
            training_step: false,
            distributed_train: None,
        }
    }

    /// Lower each `on event(...)` handler inside an actor into a standalone `HirFn`.
    ///
    /// Each handler is named `"ActorName::event_name"` and carries the full param list,
    /// return type, and lowered body so typecheck / codegen / runtime planning can see
    /// the handler's executable semantics. Durability is set by the caller (same as shell).
    pub(crate) fn lower_actor_handlers(
        &mut self,
        a: &crate::ast::decl::logic::ActorDecl,
    ) -> Vec<HirFn> {
        a.handlers
            .iter()
            .map(|h| {
                let handler_name = format!("{}::{}", a.name, h.event_name);
                let id = self.def_map.define(handler_name.clone());
                self.def_map.push_scope();
                let params = h.params.iter().map(|p| self.lower_param(p)).collect();
                let body = h.body.iter().map(|s| self.lower_stmt(s)).collect();
                self.def_map.pop_scope();
                HirFn {
                    id,
                    name: handler_name,
                    generics: vec![],
                    params,
                    return_type: h.return_type.as_ref().map(|t| self.lower_type(t)),
                    body,
                    is_async: false,
                    is_pub: false,
                    is_mobile_native: false,
                    is_pure: false,
                    is_reactive: false,
                    is_remote: false,
                    is_llm: false,
                    llm_model: None,
                    ai_structured_output: None,
                    ai_fixture: None,
                    embed: None,
                    is_deprecated: a.is_deprecated,
                    schedule_interval: None,
                    durability: None, // overwritten by caller (same as shell)
                    actor_state_fields: vec![],
                    capabilities: vec![],
                    postconditions: vec![],
                    ts_extern_module: None,
                    generated_hash: None,
                    span: h.span,
                    inference_model: None,
                    training_step: false,
                    distributed_train: None,
                }
            })
            .collect()
    }

    pub(crate) fn lower_form(&mut self, f: &FormDecl) -> HirForm {
        HirForm {
            name: f.name.clone(),
            fields: f
                .fields
                .iter()
                .map(|fd| HirFormField {
                    name: fd.name.clone(),
                    ty: self.lower_type(&fd.ty),
                    label: fd.label.clone(),
                    required: fd.required,
                    hidden: fd.hidden,
                    default: fd.default.as_ref().map(|d| self.lower_expr(d)),
                    constraints: fd
                        .constraints
                        .iter()
                        .map(|c| self.lower_field_constraint(c))
                        .collect(),
                    span: fd.span,
                })
                .collect(),
            on_submit: f.on_submit.clone(),
            success_redirect: f.success_redirect.clone(),
            error_message: f.error_message.clone(),
            span: f.span,
        }
    }

    fn lower_field_constraint(&mut self, c: &FieldConstraint) -> HirFieldConstraint {
        match c {
            FieldConstraint::Range(lo, hi) => {
                HirFieldConstraint::Range(self.lower_expr(lo), self.lower_expr(hi))
            }
            FieldConstraint::MaxLen(n) => HirFieldConstraint::MaxLen(*n),
            FieldConstraint::MinLen(n) => HirFieldConstraint::MinLen(*n),
            FieldConstraint::Pattern(p) => HirFieldConstraint::Pattern(p.clone()),
            FieldConstraint::Enum(exprs) => {
                HirFieldConstraint::Enum(exprs.iter().map(|e| self.lower_expr(e)).collect())
            }
            FieldConstraint::Custom(s) => HirFieldConstraint::Custom(s.clone()),
        }
    }

    pub(crate) fn lower_state_machine(&mut self, s: &StateMachineDecl) -> HirStateMachineDecl {
        let id = self.def_map.define(s.name.clone());
        HirStateMachineDecl {
            id,
            name: s.name.clone(),
            states: s
                .states
                .iter()
                .map(|st| HirSmState {
                    name: st.name.clone(),
                    fields: st
                        .fields
                        .iter()
                        .map(|f| HirSmField {
                            name: f.name.clone(),
                            ty: f.type_ann.as_ref().map(|t| self.lower_type(t)),
                            span: f.span,
                        })
                        .collect(),
                    is_terminal: st.is_terminal,
                    span: st.span,
                })
                .collect(),
            transitions: s
                .transitions
                .iter()
                .map(|tr| HirSmTransition {
                    event_name: tr.event_name.clone(),
                    event_params: tr.event_params.clone(),
                    from: match &tr.from {
                        SmFromPattern::Named(n) => HirSmFrom::Named(n.clone()),
                        SmFromPattern::Any => HirSmFrom::Any,
                    },
                    to_state: tr.to_state.clone(),
                    span: tr.span,
                })
                .collect(),
            is_partial: s.is_partial,
            is_pub: s.is_pub,
            span: s.span,
        }
    }
}
