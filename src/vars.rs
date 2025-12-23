use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Variable context for recipe substitution
#[derive(Debug, Default)]
pub struct VarContext {
    /// Variables from all sources, already resolved by priority
    vars: HashMap<String, String>,
}

impl VarContext {
    /// Create a new variable context with built-in variables and recipe vars
    ///
    /// Resolution order (highest to lowest priority):
    /// 1. CLI overrides (--set key=value)
    /// 2. Recipe [vars] section
    /// 3. Built-in variables (os, arch, home, recipe_dir)
    pub fn new(
        recipe_vars: &HashMap<String, String>,
        cli_overrides: &[String],
        recipe_path: Option<&Path>,
    ) -> Result<Self> {
        let mut vars = HashMap::new();

        // 1. Start with built-in variables (lowest priority)
        vars.insert("os".to_string(), std::env::consts::OS.to_string());
        vars.insert("arch".to_string(), std::env::consts::ARCH.to_string());

        if let Some(home) = dirs::home_dir() {
            vars.insert("home".to_string(), home.to_string_lossy().to_string());
        }

        if let Some(path) = recipe_path
            && let Some(parent) = path.parent()
        {
            let recipe_dir = if parent.as_os_str().is_empty() {
                ".".to_string()
            } else {
                parent.to_string_lossy().to_string()
            };
            vars.insert("recipe_dir".to_string(), recipe_dir);
        }

        // 2. Apply recipe vars (override built-ins)
        for (key, value) in recipe_vars {
            vars.insert(key.clone(), value.clone());
        }

        // 3. Apply CLI overrides (highest priority)
        for override_str in cli_overrides {
            let (key, value) = parse_key_value(override_str)
                .with_context(|| format!("Invalid --set format: {override_str}"))?;
            vars.insert(key, value);
        }

        Ok(Self { vars })
    }

    /// Substitute variables in a string
    ///
    /// Supports:
    /// - ${var} - substitute with variable value
    /// - $${literal} - escape to produce ${literal}
    /// - ${env.VAR} - substitute with environment variable
    /// - ~/path - expands to home directory
    pub fn substitute(&self, input: &str) -> Result<String> {
        let mut result = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '$' {
                if chars.peek() == Some(&'$') {
                    // Escaped: $$ -> $
                    chars.next();
                    if chars.peek() == Some(&'{') {
                        // $${...} -> ${...}
                        result.push('$');
                    } else {
                        // Just $$ -> $
                        result.push('$');
                    }
                } else if chars.peek() == Some(&'{') {
                    // Variable substitution
                    chars.next(); // consume '{'
                    let var_name: String = chars.by_ref().take_while(|&c| c != '}').collect();

                    if var_name.is_empty() {
                        return Err(anyhow::anyhow!("Empty variable name in: {input}"));
                    }

                    let value = self.resolve_var(&var_name).with_context(|| {
                        format!("Failed to resolve variable '${{{var_name}}}' in: {input}")
                    })?;
                    result.push_str(&value);
                } else {
                    // Just a $ not followed by { or $
                    result.push(c);
                }
            } else {
                result.push(c);
            }
        }

        // Apply tilde expansion to final result
        let result = self.expand_tilde(&result);

        Ok(result)
    }

    /// Expand tilde at start of string to home directory
    fn expand_tilde(&self, input: &str) -> String {
        if input.starts_with("~/")
            && let Some(home) = self.vars.get("home")
        {
            return format!("{}{}", home, &input[1..]);
        } else if input == "~"
            && let Some(home) = self.vars.get("home")
        {
            return home.clone();
        }
        input.to_string()
    }

    /// Resolve a variable name to its value
    fn resolve_var(&self, name: &str) -> Result<String> {
        // Check for env.VAR syntax
        if let Some(env_var) = name.strip_prefix("env.") {
            return std::env::var(env_var)
                .with_context(|| format!("Environment variable '{env_var}' not set"));
        }

        // Look up in our vars map
        self.vars
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Undefined variable: {name}"))
    }

    /// Get a reference to all resolved variables
    pub fn vars(&self) -> &HashMap<String, String> {
        &self.vars
    }
}

/// Parse a key=value string
fn parse_key_value(s: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!("Expected format 'key=value', got: {s}"));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_substitution() {
        let mut recipe_vars = HashMap::new();
        recipe_vars.insert("version".to_string(), "1.0.0".to_string());
        recipe_vars.insert("name".to_string(), "myapp".to_string());

        let ctx = VarContext::new(&recipe_vars, &[], None).unwrap();

        assert_eq!(
            ctx.substitute("https://example.com/${name}-${version}.zip")
                .unwrap(),
            "https://example.com/myapp-1.0.0.zip"
        );
    }

    #[test]
    fn test_builtin_vars() {
        let ctx = VarContext::new(&HashMap::new(), &[], None).unwrap();

        // Should have os and arch
        assert!(ctx.vars().contains_key("os"));
        assert!(ctx.vars().contains_key("arch"));

        let result = ctx.substitute("platform-${os}-${arch}").unwrap();
        assert!(result.starts_with("platform-"));
    }

    #[test]
    fn test_cli_override() {
        let mut recipe_vars = HashMap::new();
        recipe_vars.insert("version".to_string(), "1.0.0".to_string());

        let overrides = vec!["version=2.0.0".to_string()];
        let ctx = VarContext::new(&recipe_vars, &overrides, None).unwrap();

        assert_eq!(ctx.substitute("v${version}").unwrap(), "v2.0.0");
    }

    #[test]
    fn test_escape_sequence() {
        let ctx = VarContext::new(&HashMap::new(), &[], None).unwrap();

        assert_eq!(ctx.substitute("$${literal}").unwrap(), "${literal}");
        assert_eq!(ctx.substitute("$$foo").unwrap(), "$foo");
    }

    #[test]
    fn test_undefined_var_error() {
        let ctx = VarContext::new(&HashMap::new(), &[], None).unwrap();
        assert!(ctx.substitute("${undefined_var}").is_err());
    }

    #[test]
    fn test_no_substitution_needed() {
        let ctx = VarContext::new(&HashMap::new(), &[], None).unwrap();
        assert_eq!(
            ctx.substitute("plain string without vars").unwrap(),
            "plain string without vars"
        );
    }

    #[test]
    fn test_recipe_dir() {
        let path = Path::new("/some/path/recipe.toml");
        let ctx = VarContext::new(&HashMap::new(), &[], Some(path)).unwrap();

        assert_eq!(ctx.vars().get("recipe_dir").unwrap(), "/some/path");
    }

    #[test]
    fn test_tilde_expansion() {
        let ctx = VarContext::new(&HashMap::new(), &[], None).unwrap();
        let home = ctx.vars().get("home").unwrap().clone();

        assert_eq!(
            ctx.substitute("~/.local/bin").unwrap(),
            format!("{home}/.local/bin")
        );
        assert_eq!(ctx.substitute("~").unwrap(), home);
        // Tilde in middle of string should not expand
        assert_eq!(ctx.substitute("/path/to/~").unwrap(), "/path/to/~");
    }
}
