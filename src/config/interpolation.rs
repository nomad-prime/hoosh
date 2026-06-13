use super::error::{ConfigError, ConfigResult};
use std::path::Path;
use std::{env, fs};

const PREFIX: &str = "${env:";

pub fn interpolate(content: &str) -> ConfigResult<String> {
    let mut out = String::with_capacity(content.len());
    let mut rest = content;

    while let Some(start) = rest.find(PREFIX) {
        out.push_str(&rest[..start]);
        let after = &rest[start + PREFIX.len()..];
        let end = after.find('}').ok_or_else(|| ConfigError::BadInterpolation {
            detail: format!("unterminated '{PREFIX}…' (missing closing '}}')"),
        })?;
        let name = &after[..end];
        if name.is_empty() || !is_valid_var_name(name) {
            return Err(ConfigError::BadInterpolation {
                detail: format!("invalid variable name '{name}' in '${{env:{name}}}'"),
            });
        }
        let value = env::var(name).map_err(|_| ConfigError::MissingEnvVar {
            name: name.to_string(),
        })?;
        out.push_str(&value);
        rest = &after[end + 1..];
    }

    out.push_str(rest);
    Ok(out)
}

fn is_valid_var_name(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn load_env_file(path: &Path) {
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !is_valid_var_name(key) {
            continue;
        }
        if env::var_os(key).is_some() {
            continue;
        }
        let value = strip_quotes(value.trim());
        unsafe {
            env::set_var(key, value);
        }
    }
}

fn strip_quotes(s: &str) -> &str {
    let bytes = s.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set(key: &str, val: &str) {
        unsafe { env::set_var(key, val) }
    }
    fn unset(key: &str) {
        unsafe { env::remove_var(key) }
    }

    #[test]
    fn substitutes_simple_var() {
        let _g = ENV_LOCK.lock().unwrap();
        set("HOOSH_TEST_VAR_A", "secret123");
        let out = interpolate("api_key = \"${env:HOOSH_TEST_VAR_A}\"").unwrap();
        assert_eq!(out, "api_key = \"secret123\"");
        unset("HOOSH_TEST_VAR_A");
    }

    #[test]
    fn substitutes_mid_string() {
        let _g = ENV_LOCK.lock().unwrap();
        set("HOOSH_TEST_VAR_B", "abc");
        let out = interpolate("url = \"https://${env:HOOSH_TEST_VAR_B}.example.com\"").unwrap();
        assert_eq!(out, "url = \"https://abc.example.com\"");
        unset("HOOSH_TEST_VAR_B");
    }

    #[test]
    fn missing_var_errors() {
        let _g = ENV_LOCK.lock().unwrap();
        unset("HOOSH_TEST_MISSING");
        let err = interpolate("k = \"${env:HOOSH_TEST_MISSING}\"").unwrap_err();
        assert!(matches!(err, ConfigError::MissingEnvVar { ref name } if name == "HOOSH_TEST_MISSING"));
    }

    #[test]
    fn literal_dollar_left_alone() {
        let out = interpolate("k = \"price: $5.00\"").unwrap();
        assert_eq!(out, "k = \"price: $5.00\"");
    }

    #[test]
    fn unterminated_errors() {
        let err = interpolate("k = \"${env:FOO\"").unwrap_err();
        assert!(matches!(err, ConfigError::BadInterpolation { .. }));
    }

    #[test]
    fn invalid_name_errors() {
        let err = interpolate("k = \"${env:has-dash}\"").unwrap_err();
        assert!(matches!(err, ConfigError::BadInterpolation { .. }));
    }

    #[test]
    fn multiple_subs_in_one_line() {
        let _g = ENV_LOCK.lock().unwrap();
        set("HOOSH_TEST_X", "X");
        set("HOOSH_TEST_Y", "Y");
        let out = interpolate("k = \"${env:HOOSH_TEST_X}-${env:HOOSH_TEST_Y}\"").unwrap();
        assert_eq!(out, "k = \"X-Y\"");
        unset("HOOSH_TEST_X");
        unset("HOOSH_TEST_Y");
    }

    #[test]
    fn env_file_loads_when_unset() {
        let _g = ENV_LOCK.lock().unwrap();
        unset("HOOSH_ENVFILE_NEW");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        fs::write(&path, "HOOSH_ENVFILE_NEW=fromfile\n").unwrap();
        load_env_file(&path);
        assert_eq!(env::var("HOOSH_ENVFILE_NEW").unwrap(), "fromfile");
        unset("HOOSH_ENVFILE_NEW");
    }

    #[test]
    fn env_file_does_not_override_process_env() {
        let _g = ENV_LOCK.lock().unwrap();
        set("HOOSH_ENVFILE_WINS", "fromprocess");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        fs::write(&path, "HOOSH_ENVFILE_WINS=fromfile\n").unwrap();
        load_env_file(&path);
        assert_eq!(env::var("HOOSH_ENVFILE_WINS").unwrap(), "fromprocess");
        unset("HOOSH_ENVFILE_WINS");
    }

    #[test]
    fn env_file_strips_quotes_and_skips_comments() {
        let _g = ENV_LOCK.lock().unwrap();
        unset("HOOSH_ENVFILE_QUOTED");
        unset("HOOSH_ENVFILE_COMMENTED");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        fs::write(
            &path,
            "# a comment\nHOOSH_ENVFILE_QUOTED=\"val ue\"\n#HOOSH_ENVFILE_COMMENTED=nope\n",
        )
        .unwrap();
        load_env_file(&path);
        assert_eq!(env::var("HOOSH_ENVFILE_QUOTED").unwrap(), "val ue");
        assert!(env::var("HOOSH_ENVFILE_COMMENTED").is_err());
        unset("HOOSH_ENVFILE_QUOTED");
    }
}
