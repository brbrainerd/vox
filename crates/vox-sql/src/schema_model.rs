#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IntrospectedColumn {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub default_expr: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IntrospectedTable {
    pub name: String,
    pub columns: Vec<IntrospectedColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IntrospectedSchema {
    pub backend: String,
    pub tables: Vec<IntrospectedTable>,
}
