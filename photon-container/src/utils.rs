pub fn parse_param<T: std::str::FromStr>(s: Option<&&str>, default: T) -> T {
    s.and_then(|p| p.parse::<T>().ok()).unwrap_or(default)
}
