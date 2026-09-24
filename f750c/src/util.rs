use uuid::Uuid;

/// Trims the string after the first occurrence of the specified delimiter.
pub fn trim_after(string: &str, delimeter: char) -> String {
    if let Some(index) = string.find(delimeter) {
        string[..index].to_string()
    } else {
        string.to_string()
    }
}

pub fn random_internal_label() -> String {
    Uuid::now_v7()
        .to_string()
        .replace("-", "")
}