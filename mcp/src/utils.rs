pub fn load_api_key() -> anyhow::Result<String> {
    let config_path = if cfg!(windows) {
        dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
            .join("shuttle")
            .join("config.toml")
    } else {
        dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
            .join(".config")
            .join("shuttle")
            .join("config.toml")
    };

    let config_content = std::fs::read_to_string(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file at {:?}: {}", config_path, e))?;

    let config: toml::Value = toml::from_str(&config_content)?;

    config
        .get("api_key")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("api_key not found in config file"))
}
