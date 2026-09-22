pub fn trim_after(string: &str, delimeter: char) -> String {
    if let Some(index) = string.find(delimeter) {
        string[..index].to_string()
    } else {
        string.to_string()
    }
}