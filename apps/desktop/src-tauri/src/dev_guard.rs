use std::path::Path;

pub const PRODUCTION_BUNDLE_ID: &str = "com.foxwork.fox-stars-lab";
pub const DEV_BUNDLE_ID: &str = "com.foxwork.fox-stars-lab.dev";
pub const PRODUCTION_CREDENTIAL_SERVICE: &str = "fox-stars-lab";
pub const DEV_CREDENTIAL_SERVICE: &str = "fox-stars-lab-dev";

pub fn is_dev_identifier(identifier: &str) -> bool {
    identifier == DEV_BUNDLE_ID || identifier.ends_with(".dev")
}

pub fn credential_service_for(identifier: &str) -> &'static str {
    if is_dev_identifier(identifier) {
        DEV_CREDENTIAL_SERVICE
    } else {
        PRODUCTION_CREDENTIAL_SERVICE
    }
}

/// 正式数据目录的最后一级必须精确等于正式 bundle id，不能把 `.dev` 误判进去。
pub fn is_production_data_dir(data_dir: &Path) -> bool {
    data_dir.file_name().and_then(|name| name.to_str()) == Some(PRODUCTION_BUNDLE_ID)
}

pub fn must_refuse_production_data_dir(
    debug_build: bool,
    identifier: &str,
    data_dir: &Path,
) -> bool {
    (debug_build || is_dev_identifier(identifier)) && is_production_data_dir(data_dir)
}

pub fn refuse_if_production_data_dir(
    debug_build: bool,
    identifier: &str,
    data_dir: &Path,
) -> Result<(), String> {
    if must_refuse_production_data_dir(debug_build, identifier, data_dir) {
        Err(production_data_dir_refusal_message().to_owned())
    } else {
        Ok(())
    }
}

pub fn production_data_dir_refusal_message() -> &'static str {
    "已拒绝打开正式数据目录 ~/Library/Application Support/com.foxwork.fox-stars-lab/。开发请使用 Fox Stars Lab Dev（com.foxwork.fox-stars-lab.dev），不要启动正式版身份。"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn production_dir() -> PathBuf {
        PathBuf::from("/Users/fox/Library/Application Support/com.foxwork.fox-stars-lab")
    }

    fn dev_dir() -> PathBuf {
        PathBuf::from("/Users/fox/Library/Application Support/com.foxwork.fox-stars-lab.dev")
    }

    #[test]
    fn debug_build_refuses_production_data_dir() {
        assert!(must_refuse_production_data_dir(
            true,
            PRODUCTION_BUNDLE_ID,
            &production_dir()
        ));
    }

    #[test]
    fn dev_identifier_refuses_production_data_dir_even_in_release() {
        assert!(must_refuse_production_data_dir(
            false,
            DEV_BUNDLE_ID,
            &production_dir()
        ));
    }

    #[test]
    fn dev_data_dir_is_allowed() {
        assert!(!must_refuse_production_data_dir(
            true,
            DEV_BUNDLE_ID,
            &dev_dir()
        ));
        assert!(!is_production_data_dir(&dev_dir()));
    }

    #[test]
    fn release_production_identity_is_not_blocked_by_this_guard() {
        assert!(!must_refuse_production_data_dir(
            false,
            PRODUCTION_BUNDLE_ID,
            &production_dir()
        ));
    }

    #[test]
    fn dev_identifier_uses_dev_keychain_service() {
        assert_eq!(
            credential_service_for(DEV_BUNDLE_ID),
            DEV_CREDENTIAL_SERVICE
        );
        assert_eq!(
            credential_service_for(PRODUCTION_BUNDLE_ID),
            PRODUCTION_CREDENTIAL_SERVICE
        );
    }
}
