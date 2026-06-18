mod components;
mod crud;
mod embedding;
mod kb;
mod knowledge;
mod retention;
mod search;

pub(crate) fn sanitize_fts_query(input: &str) -> String {
    let cleaned = input
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                ' '
            }
        })
        .collect::<String>();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}
