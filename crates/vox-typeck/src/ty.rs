/// Internal type representation for the type checker.
#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Int,
    Float,
    Str,
    Bool,
    Unit,
    List(Box<Ty>),
    Option(Box<Ty>),
    Result(Box<Ty>),
    Fn(Vec<Ty>, Box<Ty>),
    Tuple(Vec<Ty>),
    Record(Vec<(String, Ty)>),
    Element,
    TypeVar(u32),
    GenericParam(u32),
    Named(String),
    Error,
    // Database types
    Database,
    Table(String, Vec<(String, Ty)>),
}
