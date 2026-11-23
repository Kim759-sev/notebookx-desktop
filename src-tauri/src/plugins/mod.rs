use crate::types::PluginDescriptor;
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path},
};

const BUILTINS: &[&str] = &[
    include_str!("../../../plugins/comfyui-studio/plugin.toml"),
    include_str!("../../../plugins/llama-cpp/plugin.toml"),
];

fn parse_manifest(source: &str, root: Option<&Path>) -> Result<PluginDescriptor, String> {
    let mut table: toml::Table = toml::from_str(source).map_err(|e| format!("{e}"))?;
    table.insert("builtin".into(), toml::Value::Boolean(root.is_none()));
    let mut plugin: PluginDescriptor = toml::Value::Table(table)
        .try_into()
        .map_err(|e| format!("{e}"))?;
    if plugin.id.is_empty()
        || !plugin
            .id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err("invalid plugin ID".into());
    }
    let script = Path::new(&plugin.scriptPath);
    if script.as_os_str().is_empty()
        || !script
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err("scriptPath must be relative to the plugin".into());
    }
    if let Some(directory) = root {
        let script = directory
            .join(script)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !script.is_file()
            || !script.starts_with(directory.canonicalize().map_err(|e| e.to_string())?)
        {
            return Err("plugin script is outside its directory or missing".into());
        }
        plugin.scriptPath = script.to_string_lossy().into_owned();
    }
    if plugin.endpoints.iter().any(|e| {
        e.port == 0
            || !e.path.starts_with('/')
            || e.healthPath.as_ref().is_some_and(|p| !p.starts_with('/'))
    }) {
        return Err("invalid plugin endpoint".into());
    }
    Ok(plugin)
}

pub fn load_all_plugins(app_data: &Path) -> Vec<PluginDescriptor> {
    let mut result: Vec<_> = BUILTINS
        .iter()
        .map(|source| parse_manifest(source, None).expect("invalid bundled plugin"))
        .collect();
    let mut ids: HashSet<_> = result.iter().map(|p| p.id.clone()).collect();
    let root = app_data.join("plugins");
    let mut directories: Vec<_> = match fs::read_dir(root) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.path())
            .collect(),
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!("Cannot list plugins: {e}");
            }
            return result;
        }
    };
    directories.sort();
    for directory in directories {
        let manifest = directory.join("plugin.toml");
        match fs::read_to_string(&manifest)
            .map_err(|e| e.to_string())
            .and_then(|source| parse_manifest(&source, Some(&directory)))
        {
            Ok(plugin) if ids.insert(plugin.id.clone()) => result.push(plugin),
            Ok(plugin) => tracing::warn!("Ignoring duplicate plugin ID {}", plugin.id),
            Err(e) => tracing::warn!("Ignoring {}: {e}", manifest.display()),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merges_local_and_builtin_manifests_without_shadowing() {
        let root = tempfile::tempdir().unwrap();
        for (name, manifest) in [
            ("x", BUILTINS[1].replacen("llama-cpp", "user-worker", 1)),
            ("duplicate", BUILTINS[1].to_owned()),
            ("broken", "[oops".into()),
            ("escape", BUILTINS[1].replace("worker.py", "../worker.py")),
        ] {
            let dir = root.path().join("plugins").join(name);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("plugin.toml"), manifest).unwrap();
            fs::write(dir.join("worker.py"), "").unwrap();
        }
        let plugins = load_all_plugins(root.path());
        assert_eq!(
            plugins.iter().map(|p| &*p.id).collect::<Vec<_>>(),
            ["comfyui-studio", "llama-cpp", "user-worker"]
        );
        assert!(plugins[..2].iter().all(|p| p.builtin && p.name.len() == 3));
        assert!(!plugins[2].builtin);
        assert!(Path::new(&plugins[2].scriptPath).is_file());
    }
}
