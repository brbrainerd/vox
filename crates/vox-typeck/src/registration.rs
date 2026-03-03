use crate::env::{ActorHandlerSig, AdtDef, Binding, BindingKind, TypeEnv, VariantDef, WorkflowSig};
use crate::ty::Ty;
use std::rc::Rc;
use vox_hir::hir::{
    HirActivity, HirActor, HirAgent, HirCollection, HirConst, HirFn, HirMessage, HirModule,
    HirTable, HirTrait, HirTypeDef, HirWorkflow, HirType,
};

/// Register all top-level declarations from an HIR module into the type environment.
///
/// This is the "Pass 1" of type checking: it makes every name visible so that
/// forward references work when bodies are checked in Pass 2.
pub fn register_hir_module(env: &mut TypeEnv, module: &HirModule) {
    for td in &module.types { register_hir_typedef(env, td); }
    for m in &module.messages { register_hir_message(env, m); }
    for t in &module.traits { register_hir_trait(env, t); }
    for f in &module.functions { register_hir_function(env, f); }
    for sf in &module.server_fns { register_fn_like(env, &sf.params, sf.return_type.as_ref(), &sf.name); }
    for t in &module.tests { register_hir_function(env, t); }
    for f in &module.fixtures { register_hir_function(env, f); }
    for h in &module.hooks { register_fn_like(env, &h.params, h.return_type.as_ref(), &h.name); }
    for p in &module.providers { register_hir_function(env, &p.func); }
    for q in &module.queries { register_hir_function(env, &q.func); }
    for m in &module.mutations { register_hir_function(env, &m.func); }
    for a in &module.actions { register_hir_function(env, &a.func); }
    for s in &module.skills { register_hir_function(env, &s.func); }
    for a in &module.agents { register_hir_function(env, &a.func); }
    for s in &module.scheduled { register_hir_function(env, &s.func); }
    for t in &module.mcp_tools { register_hir_function(env, &t.func); }
    for r in &module.mcp_resources { register_hir_function(env, &r.func); }
    for m in &module.mocks { register_hir_function(env, &m.func); }
    for a in &module.actors { register_hir_actor(env, a); }
    for w in &module.workflows { register_hir_workflow(env, w); }
    for act in &module.activities { register_hir_activity(env, act); }
    for t in &module.tables { register_hir_table(env, t); }
    for c in &module.collections { register_hir_collection(env, c); }
    for c in &module.consts { register_hir_const(env, c); }
    for na in &module.native_agents { register_hir_native_agent(env, na); }
}

/// Convert an HIR Type to an internal Ty.
pub fn resolve_hir_type(te: &HirType, env: &TypeEnv) -> Ty {
    match te {
        HirType::Named(name) => {
            if let Some(ty) = env.lookup_type(name) { return ty; }
            match name.as_str() {
                "int" => Ty::Int,
                "float" | "float64" => Ty::Float,
                "str" => Ty::Str,
                "bool" => Ty::Bool,
                "char" => Ty::Char,
                "bytes" => Ty::Bytes,
                "never" => Ty::Never,
                "Unit" => Ty::Unit,
                "Element" => Ty::Element,
                "tensor" | "Tensor" => Ty::Tensor(0),
                "NnModule" => Ty::NnModule,
                "Optimizer" => Ty::Optimizer,
                other => Ty::Named(other.to_string()),
            }
        }
        HirType::Generic(name, args) => {
            let inner_args: Vec<Ty> = args.iter().map(|a| resolve_hir_type(a, env)).collect();
            match name.as_str() {
                "list" | "List" => Ty::List(Rc::new(inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)))),
                "Option" => Ty::Option(Rc::new(inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)))),
                "Result" => Ty::Result(Rc::new(inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)))),
                "Stream" => Ty::Stream(Rc::new(inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)))),
                "Map" => {
                    let mut it = inner_args.into_iter();
                    Ty::Map(Rc::new(it.next().unwrap_or(Ty::TypeVar(0))), Rc::new(it.next().unwrap_or(Ty::TypeVar(1))))
                }
                "Set" => Ty::Set(Rc::new(inner_args.into_iter().next().unwrap_or(Ty::TypeVar(0)))),
                "Id" => {
                    let table_name = if let Some(Ty::Named(n)) = inner_args.first() { n.clone() } else { "Unknown".to_string() };
                    Ty::Id(table_name)
                }
                _ => Ty::Named(name.clone()),
            }
        }
        HirType::Function(params, ret) => {
            Ty::Fn(params.iter().map(|p| resolve_hir_type(p, env)).collect(), Rc::new(resolve_hir_type(ret, env)))
        }
        HirType::Tuple(elements) => Ty::Tuple(elements.iter().map(|e| resolve_hir_type(e, env)).collect()),
        HirType::Union(variants) => Ty::Union(variants.iter().map(|v| resolve_hir_type(v, env)).collect()),
        HirType::Map(k, v) => Ty::Map(Rc::new(resolve_hir_type(k, env)), Rc::new(resolve_hir_type(v, env))),
        HirType::Set(e) => Ty::Set(Rc::new(resolve_hir_type(e, env))),
        HirType::Intersection(l, r) => Ty::Intersection(Rc::new(resolve_hir_type(l, env)), Rc::new(resolve_hir_type(r, env))),
        HirType::Unit => Ty::Unit,
        HirType::Boxed(inner) => resolve_hir_type(inner, env),
    }
}

pub fn register_hir_typedef(env: &mut TypeEnv, td: &HirTypeDef) {
    if let Some(ref alias) = td.type_alias {
        env.define_type(td.name.clone(), resolve_hir_type(alias, env));
    } else {
        let variants: Vec<VariantDef> = td.variants.iter().map(|v| VariantDef {
            name: v.name.clone(),
            fields: v.fields.iter().map(|f| (f.0.clone(), resolve_hir_type(&f.1, env))).collect(),
        }).collect();
        env.register_type(AdtDef { name: td.name.clone(), variants }, td.is_deprecated);
    }
}

pub fn register_hir_function(env: &mut TypeEnv, f: &HirFn) {
    let param_tys: Vec<Ty> = f.params.iter()
        .map(|p| p.type_ann.as_ref().map_or(Ty::TypeVar(0), |t| resolve_hir_type(t, env)))
        .collect();
    let ret_ty = f.return_type.as_ref().map_or(Ty::Unit, |t| resolve_hir_type(t, env));
    let mut binding = Binding::new(Ty::Fn(param_tys, Rc::new(ret_ty)), false, BindingKind::Function);
    binding.is_deprecated = f.is_deprecated;
    binding.is_pure = f.is_pure;
    env.define(f.name.clone(), binding);
}

fn register_fn_like(env: &mut TypeEnv, params: &[vox_hir::hir::HirParam], ret: Option<&HirType>, name: &str) {
    let param_tys: Vec<Ty> = params.iter()
        .map(|p| p.type_ann.as_ref().map_or(Ty::TypeVar(0), |t| resolve_hir_type(t, env)))
        .collect();
    let ret_ty = ret.map_or(Ty::Unit, |t| resolve_hir_type(t, env));
    env.define(name.to_string(), Binding::new(Ty::Fn(param_tys, Rc::new(ret_ty)), false, BindingKind::Function));
}

pub fn register_hir_actor(env: &mut TypeEnv, a: &HirActor) {
    let handlers: Vec<ActorHandlerSig> = a.handlers.iter().map(|h| ActorHandlerSig {
        event_name: h.event_name.clone(),
        params: h.params.iter()
            .map(|p| (p.name.clone(), p.type_ann.as_ref().map_or(Ty::TypeVar(0), |t| resolve_hir_type(t, env))))
            .collect(),
        return_type: h.return_type.as_ref().map_or(Ty::Unit, |t| resolve_hir_type(t, env)),
    }).collect();
    env.register_actor(a.name.clone(), handlers, a.is_deprecated);
}

pub fn register_hir_workflow(env: &mut TypeEnv, w: &HirWorkflow) {
    env.register_workflow(WorkflowSig {
        name: w.name.clone(),
        params: w.params.iter()
            .map(|p| (p.name.clone(), p.type_ann.as_ref().map_or(Ty::TypeVar(0), |t| resolve_hir_type(t, env))))
            .collect(),
        return_type: w.return_type.as_ref().map_or(Ty::Unit, |t| resolve_hir_type(t, env)),
    }, w.is_deprecated);
}

pub fn register_hir_activity(env: &mut TypeEnv, a: &HirActivity) {
    let param_tys: Vec<Ty> = a.params.iter()
        .map(|p| p.type_ann.as_ref().map_or(Ty::TypeVar(0), |t| resolve_hir_type(t, env)))
        .collect();
    let ret_ty = a.return_type.as_ref().map_or(Ty::Unit, |t| resolve_hir_type(t, env));
    env.define(a.name.clone(), Binding::new(Ty::Fn(param_tys, Rc::new(ret_ty)), false, BindingKind::Activity));
}

pub fn register_hir_table(env: &mut TypeEnv, t: &HirTable) {
    let fields: Vec<(String, Ty)> = t.fields.iter().map(|f| (f.name.clone(), resolve_hir_type(&f.type_ann, env))).collect();
    env.define(t.name.clone(), Binding::new(Ty::Table(t.name.clone(), fields), false, BindingKind::Table));
}

pub fn register_hir_collection(env: &mut TypeEnv, c: &HirCollection) {
    let fields: Vec<(String, Ty)> = c.fields.iter().map(|f| (f.name.clone(), resolve_hir_type(&f.type_ann, env))).collect();
    env.define(c.name.clone(), Binding::new(Ty::Collection(c.name.clone(), fields), false, BindingKind::Variable));
}

fn register_hir_const(env: &mut TypeEnv, c: &HirConst) {
    let ty = c.type_ann.as_ref().map_or(Ty::TypeVar(0), |t| resolve_hir_type(t, env));
    let mut binding = Binding::new(ty, false, BindingKind::Variable);
    binding.is_deprecated = c.is_deprecated;
    env.define(c.name.clone(), binding);
}

fn register_hir_trait(env: &mut TypeEnv, t: &HirTrait) {
    for method in &t.methods {
        let param_tys: Vec<Ty> = method.params.iter()
            .map(|p| p.type_ann.as_ref().map_or(Ty::TypeVar(0), |t| resolve_hir_type(t, env)))
            .collect();
        let ret_ty = method.return_type.as_ref().map_or(Ty::Unit, |t| resolve_hir_type(t, env));
        env.define(format!("{}::{}", t.name, method.name), Binding::new(Ty::Fn(param_tys, Rc::new(ret_ty)), false, BindingKind::Function));
    }
}

fn register_hir_message(env: &mut TypeEnv, m: &HirMessage) {
    let fields: Vec<(String, Ty)> = m.fields.iter().map(|(name, ty)| (name.clone(), resolve_hir_type(ty, env))).collect();
    env.define_type(m.name.clone(), Ty::Record(fields));
}

fn register_hir_native_agent(env: &mut TypeEnv, a: &HirAgent) {
    let handlers: Vec<ActorHandlerSig> = a.handlers.iter().map(|h| ActorHandlerSig {
        event_name: h.event_name.clone(),
        params: h.params.iter()
            .map(|p| (p.name.clone(), p.type_ann.as_ref().map_or(Ty::TypeVar(0), |t| resolve_hir_type(t, env))))
            .collect(),
        return_type: h.return_type.as_ref().map_or(Ty::Unit, |t| resolve_hir_type(t, env)),
    }).collect();
    env.register_actor(a.name.clone(), handlers, a.is_deprecated);
}
