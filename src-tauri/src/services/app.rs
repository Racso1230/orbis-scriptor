use crate::models::app::AppInfo;

/// Builds the `AppInfo` shown on the welcome screen.
///
/// `&str` means "borrow a string slice": the caller keeps ownership and we only
/// read it. `.to_owned()` copies it into a `String` that the struct owns.
pub fn app_info(name: &str, version: &str) -> AppInfo {
    AppInfo {
        name: name.to_owned(),
        version: version.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_name_and_version() {
        let info = app_info("Orbis Scriptor", "0.1.0");
        assert_eq!(
            info,
            AppInfo {
                name: "Orbis Scriptor".into(),
                version: "0.1.0".into(),
            }
        );
    }
}
