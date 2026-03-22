use crate::ENGINE_API_VERSION;

pub fn version_major(version: &str) -> Option<u64> {
    version
        .trim()
        .split('.')
        .next()
        .and_then(|major| major.parse::<u64>().ok())
}

pub fn engine_api_major_compatible(actual_version: &str, required_major: u64) -> bool {
    version_major(actual_version).is_some_and(|major| major >= required_major)
}

pub fn expected_engine_api_major() -> Option<u64> {
    version_major(ENGINE_API_VERSION)
}

pub(crate) fn protocol_major_matches(actual: &str, expected: &str) -> bool {
    let a = actual.split('.').next().unwrap_or(actual);
    let e = expected.split('.').next().unwrap_or(expected);
    a == e
}
