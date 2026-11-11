//! Data-only theme packages. No package code is executed by the host.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{CString, OsStr},
    fs::{self, File},
    io::{self, Read, Write},
    os::unix::{
        ffi::OsStrExt,
        io::{AsRawFd, FromRawFd},
    },
    path::{Component, Path, PathBuf},
};

const MANIFEST_LIMIT: usize = 64 * 1024;
const DATA_LIMIT: usize = 1024 * 1024;
const ASSET_LIMIT: usize = 4 * 1024 * 1024;
const OUTPUT_LIMIT: usize = 4 * 1024 * 1024;
const MAX_OUTPUTS: usize = 32;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: u32,
    pub target: String,
    #[serde(default)]
    pub target_versions: Vec<String>,
    pub outputs: Vec<Output>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub path: String,
    pub template: String,
    pub format: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub help: String,
    pub default: String,
    #[serde(default)]
    pub alpha: bool,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub links: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub id: String,
    pub title: String,
    pub width: f32,
    pub height: f32,
    pub nodes: Vec<Node>,
}
fn default_enabled() -> bool {
    true
}
fn default_font_size() -> f32 {
    14.0
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub rect: [f32; 4],
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub text_role: Option<String>,
    #[serde(default)]
    pub border_role: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub asset: Option<String>,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}
#[derive(Clone, Debug)]
pub struct Plugin {
    pub manifest: Manifest,
    pub roles: Vec<Role>,
    pub scenes: Vec<Scene>,
    path: PathBuf,
    templates: BTreeMap<String, String>,
}
#[derive(Clone, Debug)]
pub struct PluginMetadata {
    pub manifest: Manifest,
    pub path: PathBuf,
}
#[derive(Clone, Debug)]
pub struct RenderedFile {
    pub path: String,
    pub content: String,
}

fn ident(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.as_bytes()[0].is_ascii_lowercase()
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}
fn parts(path: &str) -> Result<Vec<&OsStr>, String> {
    let p = Path::new(path);
    let mut out = Vec::new();
    for c in p.components() {
        match c {
            Component::Normal(x) if !x.is_empty() => out.push(x),
            _ => return Err(format!("unsafe path: {path}")),
        }
    }
    if out.is_empty()
        || path.len() > 1024
        || path.contains('\0')
        || path.contains('\\')
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err(format!("unsafe path: {path}"));
    }
    Ok(out)
}
fn cstr(s: &OsStr) -> Result<CString, String> {
    CString::new(s.as_bytes()).map_err(|_| "NUL in path".into())
}
fn open_root(path: &Path) -> Result<File, String> {
    // Walk from / or cwd so no component, including the root itself, follows a symlink.
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    let mut dir = File::open("/").map_err(|e| e.to_string())?;
    for c in absolute.components() {
        match c {
            Component::RootDir => (),
            Component::Normal(n) => {
                dir = open_at(&dir, n, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
            }
            _ => return Err(format!("unsafe directory: {}", path.display())),
        }
    }
    Ok(dir)
}
fn open_at(dir: &File, name: &OsStr, flags: i32, mode: u32) -> Result<File, String> {
    let name = cstr(name)?;
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            mode,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error().to_string());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn parent_at(root: &File, path: &str) -> Result<(File, String), String> {
    let ps = parts(path)?;
    let mut dir = root.try_clone().map_err(|e| e.to_string())?;
    for p in &ps[..ps.len() - 1] {
        dir = open_at(&dir, p, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
    }
    Ok((dir, ps.last().unwrap().to_string_lossy().into_owned()))
}
fn read_at(root: &File, path: &str, limit: usize) -> Result<Vec<u8>, String> {
    let (dir, name) = parent_at(root, path).map_err(|e| format!("{path}: {e}"))?;
    let file = open_at(
        &dir,
        OsStr::new(&name),
        libc::O_RDONLY | libc::O_NONBLOCK,
        0,
    )
    .map_err(|e| format!("{path}: {e}"))?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > limit as u64 {
        return Err(format!("{path}: not a regular file or too large"));
    }
    let mut data = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() > limit {
        return Err(format!("{path}: too large"));
    }
    Ok(data)
}
fn json_at<T: for<'de> Deserialize<'de>>(
    root: &File,
    path: &str,
    limit: usize,
) -> Result<T, String> {
    serde_json::from_slice(&read_at(root, path, limit)?).map_err(|e| format!("{path}: {e}"))
}
fn validate_manifest(m: &Manifest) -> Result<(), String> {
    if !ident(&m.id)
        || m.name.trim().is_empty()
        || m.version.trim().is_empty()
        || m.target.trim().is_empty()
        || m.api_version != 1
        || m.target_versions.is_empty()
        || m.target_versions.iter().any(|v| v.trim().is_empty())
    {
        return Err("invalid manifest identity, target, version or unsupported API version".into());
    }
    if m.outputs.is_empty() || m.outputs.len() > MAX_OUTPUTS {
        return Err("invalid output count".into());
    }
    let mut paths = BTreeSet::new();
    for o in &m.outputs {
        parts(&o.path)?;
        parts(&o.template)?;
        if !o.template.starts_with("templates/")
            || !o.template.ends_with(".j2")
            || !matches!(o.format.as_str(), "ghostty" | "json" | "kde" | "text")
            || !paths.insert(&o.path)
        {
            return Err(format!("invalid output declaration: {}", o.path));
        }
    }
    Ok(())
}
fn discovery_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let user_data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|p| p.join(".local/share"))
        });
    if let Some(path) = user_data {
        roots.push(path.join("swatch-board/plugins"));
    }
    let dirs = std::env::var_os("XDG_DATA_DIRS")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    roots.extend(
        std::env::split_paths(&dirs)
            .filter(|p| p.is_absolute())
            .map(|p| p.join("swatch-board/plugins")),
    );
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin) = exe.parent() {
            roots.push(bin.join("../share/swatch-board/plugins"));
        }
    }
    #[cfg(debug_assertions)]
    roots.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins"));
    let mut seen = BTreeSet::new();
    roots
        .into_iter()
        .map(|p| {
            let mut normalized = PathBuf::new();
            for c in p.components() {
                match c {
                    Component::ParentDir => {
                        normalized.pop();
                    }
                    Component::CurDir => (),
                    other => normalized.push(other.as_os_str()),
                }
            }
            normalized
        })
        .filter(|p| p.exists() && seen.insert(p.clone()))
        .collect()
}
/// Discover only manifests. Duplicate IDs are all rejected, not shadowed.
pub fn discover() -> (Vec<PluginMetadata>, Vec<String>) {
    let mut found: BTreeMap<String, Vec<PluginMetadata>> = BTreeMap::new();
    let mut errors = Vec::new();
    for root in discovery_roots() {
        let entries = match fs::read_dir(&root) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("{}: {e}", root.display()));
                continue;
            }
        };
        let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            let result = open_root(&path).and_then(|dir| {
                let m: Manifest = json_at(&dir, "manifest.json", MANIFEST_LIMIT)?;
                validate_manifest(&m)?;
                Ok(m)
            });
            match result {
                Ok(manifest) => found
                    .entry(manifest.id.clone())
                    .or_default()
                    .push(PluginMetadata { manifest, path }),
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
    }
    let mut plugins = Vec::new();
    for (id, mut group) in found {
        if group.len() == 1 {
            plugins.push(group.remove(0));
        } else {
            errors.push(format!(
                "duplicate plugin ID {id}: {}",
                group
                    .iter()
                    .map(|p| p.path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    (plugins, errors)
}
fn color(value: &str, alpha: bool) -> Result<String, String> {
    let b = value.as_bytes();
    if b.first() != Some(&b'#')
        || !(b.len() == 7 || b.len() == 9)
        || !b[1..].iter().all(u8::is_ascii_hexdigit)
    {
        return Err(format!("invalid HEX color: {value}"));
    }
    if !alpha && b.len() == 9 {
        if !value[7..9].eq_ignore_ascii_case("ff") {
            return Err(format!("nonopaque color for opaque role: {value}"));
        }
        return Ok(value[..7].to_ascii_uppercase());
    }
    Ok(value.to_ascii_uppercase())
}
fn png_data(data: &[u8]) -> Result<(), String> {
    let decoder = png::Decoder::new(data);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("invalid PNG: {e}"))?;
    let info = reader.info();
    if info.width == 0
        || info.height == 0
        || info.width > 2048
        || info.height > 2048
        || reader.output_buffer_size() > 16 * 1024 * 1024
    {
        return Err("PNG dimensions too large".into());
    }
    let mut buf = vec![0; reader.output_buffer_size()];
    reader
        .next_frame(&mut buf)
        .map_err(|e| format!("invalid PNG: {e}"))?;
    Ok(())
}
/// Load and validate all package data and referenced PNG assets.
pub fn load(path: &Path) -> Result<Plugin, String> {
    let root = open_root(path)?;
    let manifest: Manifest = json_at(&root, "manifest.json", MANIFEST_LIMIT)?;
    validate_manifest(&manifest)?;
    let roles: Vec<Role> = json_at(&root, "roles.json", DATA_LIMIT)?;
    let scenes: Vec<Scene> = json_at(&root, "scenes.json", DATA_LIMIT)?;
    if roles.is_empty() || roles.len() > 1024 || scenes.is_empty() || scenes.len() > 256 {
        return Err("invalid role or scene count".into());
    }
    let mut role_ids = BTreeSet::new();
    for r in &roles {
        if !ident(&r.id) || r.title.is_empty() || !role_ids.insert(r.id.clone()) {
            return Err(format!("invalid or duplicate role {}", r.id));
        }
        color(&r.default, r.alpha)?;
    }
    let mut scene_ids = BTreeSet::new();
    for s in &scenes {
        if !ident(&s.id)
            || !scene_ids.insert(s.id.clone())
            || !s.width.is_finite()
            || !s.height.is_finite()
            || s.width <= 0.0
            || s.height <= 0.0
            || s.width > 16384.0
            || s.height > 16384.0
            || s.nodes.len() > 4096
        {
            return Err(format!("invalid scene {}", s.id));
        }
    }
    for s in &scenes {
        let mut ids = BTreeSet::new();
        for n in &s.nodes {
            if !ident(&n.id)
                || !ids.insert(&n.id)
                || !matches!(
                    n.kind.as_str(),
                    "rect"
                        | "window"
                        | "dialog"
                        | "list"
                        | "tab"
                        | "text"
                        | "syntax"
                        | "icon"
                        | "image"
                        | "background"
                        | "border"
                )
                || n.rect.iter().any(|v| !v.is_finite() || v.abs() > 65536.0)
                || n.rect[2] < 0.0
                || n.rect[3] < 0.0
                || !n.font_size.is_finite()
                || n.font_size <= 0.0
                || n.font_size > 512.0
            {
                return Err(format!("invalid node {} in {}", n.id, s.id));
            }
            for r in [&n.role, &n.text_role, &n.border_role]
                .into_iter()
                .flatten()
            {
                if !role_ids.contains(r) {
                    return Err(format!("node {} references unknown role {r}", n.id));
                }
            }
            if let Some(t) = &n.target {
                if !scene_ids.contains(t) {
                    return Err(format!("node {} references unknown scene {t}", n.id));
                }
            }
            if let Some(a) = &n.asset {
                if !a.starts_with("assets/") || !a.ends_with(".png") {
                    return Err(format!("unsupported asset {a}"));
                }
                png_data(&read_at(&root, a, ASSET_LIMIT)?)?;
            }
        }
    }
    let mut templates = BTreeMap::new();
    for o in &manifest.outputs {
        let data = read_at(&root, &o.template, DATA_LIMIT)?;
        let text = String::from_utf8(data).map_err(|e| e.to_string())?;
        templates.insert(o.template.clone(), text);
    }
    Ok(Plugin {
        manifest,
        roles,
        scenes,
        path: path.to_path_buf(),
        templates,
    })
}
impl Plugin {
    pub fn asset(&self, path: &str) -> Result<Vec<u8>, String> {
        if !self
            .scenes
            .iter()
            .flat_map(|s| &s.nodes)
            .any(|n| n.asset.as_deref() == Some(path))
        {
            return Err("asset is not referenced by this plugin".into());
        }
        let data = read_at(&open_root(&self.path)?, path, ASSET_LIMIT)?;
        png_data(&data)?;
        Ok(data)
    }
    pub fn resolve(
        &self,
        overrides: &BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, String>, String> {
        for key in overrides.keys() {
            if !self.roles.iter().any(|r| &r.id == key) {
                return Err(format!("unknown role {key}"));
            }
        }
        self.roles
            .iter()
            .map(|r| {
                Ok((
                    r.id.clone(),
                    color(overrides.get(&r.id).unwrap_or(&r.default), r.alpha)?,
                ))
            })
            .collect()
    }
    pub fn render(&self, values: &BTreeMap<String, String>) -> Result<Vec<RenderedFile>, String> {
        let roles = self.resolve(values)?;
        let mut env = minijinja::Environment::new();
        env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
        env.set_fuel(Some(100_000));
        env.set_keep_trailing_newline(true);
        env.add_filter("hex", |s: String| -> Result<String, minijinja::Error> {
            color(&s, true)
                .map_err(|e| minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, e))
        });
        env.add_filter(
            "hex_no_hash",
            |s: String| -> Result<String, minijinja::Error> {
                color(&s, true)
                    .map(|c| c[1..].to_owned())
                    .map_err(|e| minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, e))
            },
        );
        env.add_filter("rgb_csv", |s: String| -> Result<String, minijinja::Error> {
            let c = color(&s, true)
                .map_err(|e| minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, e))?;
            Ok(format!(
                "{},{},{}",
                u8::from_str_radix(&c[1..3], 16).unwrap(),
                u8::from_str_radix(&c[3..5], 16).unwrap(),
                u8::from_str_radix(&c[5..7], 16).unwrap()
            ))
        });
        for (name, text) in &self.templates {
            env.add_template(name, text).map_err(|e| e.to_string())?;
        }
        let mut result = Vec::new();
        for o in &self.manifest.outputs {
            let mut out = LimitedWriter(Vec::new());
            env.get_template(&o.template)
                .map_err(|e| e.to_string())?
                .render_captured_to(minijinja::context! { roles => &roles }, &mut out)
                .map_err(|e| format!("{}: {e}", o.path))?;
            let content = String::from_utf8(out.0).map_err(|e| e.to_string())?;
            validate_output(&o.format, &content)?;
            result.push(RenderedFile {
                path: o.path.clone(),
                content,
            });
        }
        Ok(result)
    }
}
struct LimitedWriter(Vec<u8>);
impl Write for LimitedWriter {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if self.0.len().saturating_add(b.len()) > OUTPUT_LIMIT {
            return Err(io::Error::other("rendered output too large"));
        }
        self.0.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn validate_output(format: &str, text: &str) -> Result<(), String> {
    match format {
        "text" if !text.trim().is_empty() => {}
        "json" => {
            let _: serde_json::Value =
                serde_json::from_str(text).map_err(|e| format!("invalid JSON output: {e}"))?;
        }
        "ghostty" | "kde" => {
            let mut section = false;
            let mut entries = 0;
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                    continue;
                }
                if format == "kde" && line.starts_with('[') && line.ends_with(']') && line.len() > 2
                {
                    section = true;
                    continue;
                }
                let Some((key, value)) = line.split_once('=') else {
                    return Err(format!("invalid {format} output line: {line}"));
                };
                if key.trim().is_empty() || value.trim().is_empty() || (format == "kde" && !section)
                {
                    return Err(format!("invalid {format} output line: {line}"));
                }
                entries += 1;
            }
            if entries == 0 {
                return Err(format!("empty {format} output"));
            }
        }
        _ => return Err(format!("unsupported or empty output format {format}")),
    }
    Ok(())
}
/// Export staged files under a chosen existing directory. No path component follows symlinks.
pub fn export(destination: &Path, files: &[RenderedFile], overwrite: bool) -> Result<(), String> {
    if files.is_empty() || files.len() > MAX_OUTPUTS {
        return Err("invalid output count".into());
    }
    let root = open_root(destination)?;
    let mut seen = BTreeSet::new();
    let mut staged: Vec<(File, String, String)> = Vec::new();
    let result = (|| -> Result<(), String> {
        for (i, file) in files.iter().enumerate() {
            if file.content.len() > OUTPUT_LIMIT || !seen.insert(&file.path) {
                return Err("duplicate or oversized output".into());
            }
            let (dir, name) = parent_at(&root, &file.path)?;
            // Refuse symlinks and non-regular targets even when overwrite was requested.
            let cname = cstr(OsStr::new(&name))?;
            let mut st: libc::stat = unsafe { std::mem::zeroed() };
            let status = unsafe {
                libc::fstatat(
                    dir.as_raw_fd(),
                    cname.as_ptr(),
                    &mut st,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            };
            if status == 0 {
                if st.st_mode & libc::S_IFMT != libc::S_IFREG || !overwrite {
                    return Err(format!(
                        "output already exists or is not a regular file: {}",
                        file.path
                    ));
                }
            } else if io::Error::last_os_error().raw_os_error() != Some(libc::ENOENT) {
                return Err(io::Error::last_os_error().to_string());
            }
            let mut temp = None;
            for nonce in 0..128 {
                let t = format!(".swatch-board-{}-{i}-{nonce}.tmp", std::process::id());
                // A requested output must never alias a staging file: rename or cleanup
                // could otherwise replace another output's contents or delete it.
                if files
                    .iter()
                    .any(|f| Path::new(&f.path).file_name() == Some(OsStr::new(&t)))
                {
                    continue;
                }
                match open_at(
                    &dir,
                    OsStr::new(&t),
                    libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
                    0o600,
                ) {
                    Ok(f) => {
                        temp = Some((t, f));
                        break;
                    }
                    Err(_) => continue,
                }
            }
            let (tmp, mut handle) = temp.ok_or("cannot create staging file")?;
            staged.push((dir, tmp, name));
            handle
                .write_all(file.content.as_bytes())
                .and_then(|_| handle.sync_all())
                .map_err(|e| e.to_string())?;
        }
        for (dir, tmp, name) in &staged {
            let t = cstr(OsStr::new(tmp))?;
            let n = cstr(OsStr::new(name))?;
            let flags = if overwrite { 0 } else { libc::RENAME_NOREPLACE };
            let rc = unsafe {
                libc::renameat2(
                    dir.as_raw_fd(),
                    t.as_ptr(),
                    dir.as_raw_fd(),
                    n.as_ptr(),
                    flags,
                )
            };
            if rc != 0 {
                return Err(format!("export {name}: {}", io::Error::last_os_error()));
            }
        }
        Ok(())
    })();
    for (dir, tmp, _) in &staged {
        if let Ok(t) = cstr(OsStr::new(tmp)) {
            unsafe {
                libc::unlinkat(dir.as_raw_fd(), t.as_ptr(), 0);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "swatch-theme-test-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(p.join("templates")).unwrap();
            Self(p)
        }
        fn put(&self, name: &str, value: &str) {
            fs::write(self.0.join(name), value).unwrap();
        }
        fn fixture(&self, template: &str) {
            self.put("manifest.json", r#"{"id":"fixture","name":"Fixture","version":"1","api_version":1,"target":"ghostty","target_versions":["1"],"outputs":[{"path":"theme","template":"templates/theme.j2","format":"ghostty"}]}"#);
            self.put(
                "roles.json",
                r##"[{"id":"background","title":"Background","default":"#123456","alpha":false}]"##,
            );
            self.put("scenes.json", r#"[{"id":"main","title":"Main","width":100,"height":100,"nodes":[{"id":"panel","kind":"rect","rect":[0,0,100,100],"role":"background"}]}]"#);
            self.put("templates/theme.j2", template);
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn resolves_and_renders_strictly() {
        let t = Temp::new();
        t.fixture("background = {{ roles.background | hex }}\n");
        let p = load(&t.0).unwrap();
        let mut values = BTreeMap::new();
        values.insert("background".into(), "#abcdefFF".into());
        assert_eq!(
            p.render(&values).unwrap()[0].content,
            "background = #ABCDEF\n"
        );
        values.insert("background".into(), "#abcdef80".into());
        assert!(p.resolve(&values).is_err());
        values.insert("unknown".into(), "#abcdef".into());
        assert!(p.resolve(&values).is_err());
        t.put("templates/theme.j2", "background = {{ roles.missing }}\n");
        assert!(load(&t.0).unwrap().render(&BTreeMap::new()).is_err());
    }
    #[test]
    fn rejects_bad_references_and_symlinked_templates() {
        let t = Temp::new();
        t.fixture("background = {{ roles.background }}");
        t.put("scenes.json", r#"[{"id":"main","title":"Main","width":100,"height":100,"nodes":[{"id":"panel","kind":"rect","rect":[0,0,1,1],"role":"missing"}]}]"#);
        assert!(load(&t.0).is_err());
        t.fixture("background = {{ roles.background }}");
        fs::remove_file(t.0.join("templates/theme.j2")).unwrap();
        std::os::unix::fs::symlink("../roles.json", t.0.join("templates/theme.j2")).unwrap();
        assert!(load(&t.0).is_err());
    }
    #[test]
    fn export_protects_existing_and_rejects_traversal() {
        let t = Temp::new();
        let f = RenderedFile {
            path: "theme".into(),
            content: "one".into(),
        };
        export(&t.0, std::slice::from_ref(&f), false).unwrap();
        assert!(export(&t.0, std::slice::from_ref(&f), false).is_err());
        let changed = RenderedFile {
            path: "theme".into(),
            content: "two".into(),
        };
        export(&t.0, &[changed], true).unwrap();
        assert_eq!(fs::read_to_string(t.0.join("theme")).unwrap(), "two");
        assert!(
            export(
                &t.0,
                &[RenderedFile {
                    path: "../escape".into(),
                    content: "bad".into()
                }],
                true
            )
            .is_err()
        );
        std::os::unix::fs::symlink("theme", t.0.join("link")).unwrap();
        assert!(
            export(
                &t.0,
                &[RenderedFile {
                    path: "link".into(),
                    content: "bad".into()
                }],
                true
            )
            .is_err()
        );
    }
    #[test]
    fn loader_rejects_nul_in_declared_output_path() {
        let t = Temp::new();
        t.fixture("background = {{ roles.background }}");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(t.0.join("manifest.json")).unwrap()).unwrap();
        manifest["outputs"][0]["path"] = "theme\0hidden".into();
        t.put("manifest.json", &serde_json::to_string(&manifest).unwrap());
        assert!(load(&t.0).is_err(), "an unexportable NUL path was accepted");
    }
    #[test]
    fn export_preserves_outputs_named_like_staging_files() {
        for overwrite in [false, true] {
            for staging_index in [0, 1] {
                let t = Temp::new();
                let files = [
                    RenderedFile {
                        path: format!(".swatch-board-{}-{staging_index}-0.tmp", std::process::id()),
                        content: "first".into(),
                    },
                    RenderedFile {
                        path: "theme".into(),
                        content: "second".into(),
                    },
                ];
                export(&t.0, &files, overwrite).unwrap();
                for file in &files {
                    assert_eq!(
                        fs::read_to_string(t.0.join(&file.path)).unwrap(),
                        file.content
                    );
                }
                assert_eq!(fs::read_dir(&t.0).unwrap().count(), 3);
            }
        }
    }
    #[test]
    fn rejects_unbounded_templates_and_invalid_output() {
        let t = Temp::new();
        t.fixture("{% for x in range(100000000) %}x{% endfor %}");
        assert!(load(&t.0).unwrap().render(&BTreeMap::new()).is_err());
        t.fixture("not a valid configuration");
        assert!(load(&t.0).unwrap().render(&BTreeMap::new()).is_err());
    }
    #[test]
    fn bundled_package_golden_outputs() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        for name in ["ghostty", "zed", "kde"] {
            let package = root.join(name);
            let plugin = load(&fs::canonicalize(&package).unwrap()).unwrap();
            let values: BTreeMap<String, String> = serde_json::from_slice(
                &fs::read(package.join("tests/sample-palette.json")).unwrap(),
            )
            .unwrap();
            let rendered = plugin.render(&values).unwrap();
            assert_eq!(rendered.len(), 1);
            assert_eq!(
                rendered[0].content,
                fs::read_to_string(package.join("tests/expected-theme.txt")).unwrap(),
                "{name}"
            );
        }
    }
}
