use anyhow::{Context, Result};
use gray_matter::Matter;
use gray_matter::engine::YAML;
use indexmap::IndexMap;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: String,
    #[serde(default)]
    compatibility: Option<String>,
    #[serde(rename = "allowed-tools", default)]
    allowed_tools: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub instructions: Option<String>,
    pub compatibility: Option<String>,
    pub allowed_tools: Option<String>,
}

impl Skill {
    fn from_dir(dir: &Path) -> Result<Self> {
        let skill_md = dir.join("SKILL.md");
        let content = fs::read_to_string(&skill_md)
            .with_context(|| format!("Failed to read {}", skill_md.display()))?;

        let matter = Matter::<YAML>::new();
        let parsed = matter
            .parse_with_struct::<SkillFrontmatter>(&content)
            .with_context(|| format!("Failed to parse frontmatter in {}", skill_md.display()))?;

        Ok(Skill {
            name: parsed.data.name,
            description: parsed.data.description,
            path: dir.to_path_buf(),
            instructions: Some(parsed.content.trim().to_string()),
            compatibility: parsed.data.compatibility,
            allowed_tools: parsed.data.allowed_tools,
        })
    }

    fn from_legacy_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let description = extract_description(&content);

        Ok(Skill {
            name,
            description,
            path: path.to_path_buf(),
            instructions: None,
            compatibility: None,
            allowed_tools: None,
        })
    }

    pub fn entry_point(&self) -> PathBuf {
        if self.path.is_dir() {
            self.path.join("SKILL.md")
        } else {
            self.path.clone()
        }
    }
}

fn extract_description(content: &str) -> String {
    content
        .lines()
        .find_map(|line| {
            let line = line.trim();
            if line.starts_with('#') && !line.starts_with("#!/") {
                Some(line.trim_start_matches('#').trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

pub struct SkillManager {
    roots: Vec<PathBuf>,
}

impl SkillManager {
    pub fn new() -> Self {
        SkillManager { roots: vec![] }
    }

    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        SkillManager { roots }
    }

    pub fn discover_skills(&self) -> Result<Vec<Skill>> {
        let mut skills: IndexMap<String, Skill> = IndexMap::new();

        for root in &self.roots {
            if !root.exists() {
                continue;
            }

            let entries = match fs::read_dir(root) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();

                if path.is_dir() {
                    if path.join("SKILL.md").exists() {
                        match Skill::from_dir(&path) {
                            Ok(skill) => {
                                skills.insert(skill.name.clone(), skill);
                            }
                            Err(e) => {
                                eprintln!(
                                    "Warning: Failed to load skill {}: {}",
                                    path.display(),
                                    e
                                );
                            }
                        }
                    }
                } else if self.is_legacy_skill(&path) {
                    match Skill::from_legacy_file(&path) {
                        Ok(skill) => {
                            skills.insert(skill.name.clone(), skill);
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to load skill {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }

        Ok(skills.into_values().collect())
    }

    fn is_legacy_skill(&self, path: &Path) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = path.metadata() {
                return metadata.permissions().mode() & 0o111 != 0;
            }
            false
        }

        #[cfg(not(unix))]
        {
            let _ = path;
            true
        }
    }

    pub fn get_skills_summary(&self, skills: &[Skill]) -> String {
        if skills.is_empty() {
            return String::new();
        }

        let mut summary = "<available_skills>\n".to_string();
        summary.push_str("Skills available for this project. Check relevant skills before falling back to raw bash commands. For folder-based skills, read SKILL.md at the listed path for full instructions.\n\n");

        for skill in skills {
            let entry = skill.entry_point();
            summary.push_str(&format!(
                "- **{}**: path: {}\n",
                skill.name,
                entry.display()
            ));
            if !skill.description.is_empty() {
                summary.push_str(&format!("  {}\n", skill.description));
            } else {
                summary.push_str("  (no description)\n");
            }
        }

        summary.push_str("</available_skills>");
        summary
    }
}

impl Default for SkillManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_skills_dir(tmp: &TempDir) -> PathBuf {
        let dir = tmp.path().join(".hoosh").join("skills");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_executable(path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms).unwrap();
        }
    }

    #[test]
    fn discover_returns_empty_when_no_roots() {
        let manager = SkillManager::new();
        let skills = manager.discover_skills().unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn discover_returns_empty_for_missing_root() {
        let manager = SkillManager::with_roots(vec![PathBuf::from("/nonexistent/path")]);
        let skills = manager.discover_skills().unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn discover_legacy_executable_script() -> Result<()> {
        let tmp = TempDir::new()?;
        let skills_dir = make_skills_dir(&tmp);
        let script = skills_dir.join("deploy.sh");
        fs::write(&script, "#!/bin/bash\n# Deploy the app\necho ok")?;
        make_executable(&script);

        let manager = SkillManager::with_roots(vec![skills_dir]);
        let skills = manager.discover_skills()?;

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "deploy");
        assert_eq!(skills[0].description, "Deploy the app");
        assert!(skills[0].instructions.is_none());
        Ok(())
    }

    #[test]
    fn discover_skill_md_folder() -> Result<()> {
        let tmp = TempDir::new()?;
        let skills_dir = make_skills_dir(&tmp);
        let skill_dir = skills_dir.join("pdf-processing");
        fs::create_dir_all(&skill_dir)?;
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: pdf-processing\ndescription: Extract text from PDFs.\n---\n\nRead the PDF and extract.",
        )?;

        let manager = SkillManager::with_roots(vec![skills_dir]);
        let skills = manager.discover_skills()?;

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "pdf-processing");
        assert_eq!(skills[0].description, "Extract text from PDFs.");
        assert_eq!(
            skills[0].instructions.as_deref(),
            Some("Read the PDF and extract.")
        );
        Ok(())
    }

    #[test]
    fn discover_skill_md_optional_fields() -> Result<()> {
        let tmp = TempDir::new()?;
        let skills_dir = make_skills_dir(&tmp);
        let skill_dir = skills_dir.join("git-ops");
        fs::create_dir_all(&skill_dir)?;
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: git-ops\ndescription: Git operations.\ncompatibility: requires git\nallowed-tools: Bash(git:*)\n---\nRun git commands.",
        )?;

        let manager = SkillManager::with_roots(vec![skills_dir]);
        let skills = manager.discover_skills()?;

        assert_eq!(skills[0].compatibility.as_deref(), Some("requires git"));
        assert_eq!(skills[0].allowed_tools.as_deref(), Some("Bash(git:*)"));
        Ok(())
    }

    #[test]
    fn skill_md_with_bad_frontmatter_is_skipped() -> Result<()> {
        let tmp = TempDir::new()?;
        let skills_dir = make_skills_dir(&tmp);
        let skill_dir = skills_dir.join("broken");
        fs::create_dir_all(&skill_dir)?;
        fs::write(skill_dir.join("SKILL.md"), "no frontmatter at all")?;

        let manager = SkillManager::with_roots(vec![skills_dir]);
        let skills = manager.discover_skills()?;
        assert!(skills.is_empty());
        Ok(())
    }

    #[test]
    fn local_root_wins_over_central_on_name_collision() -> Result<()> {
        let central = TempDir::new()?;
        let local = TempDir::new()?;

        let central_skills = central.path().join("skills");
        let local_skills = local.path().join("skills");
        fs::create_dir_all(&central_skills)?;
        fs::create_dir_all(&local_skills)?;

        let central_skill = central_skills.join("build");
        fs::create_dir_all(&central_skill)?;
        fs::write(
            central_skill.join("SKILL.md"),
            "---\nname: build\ndescription: Central build.\n---\n",
        )?;

        let local_skill = local_skills.join("build");
        fs::create_dir_all(&local_skill)?;
        fs::write(
            local_skill.join("SKILL.md"),
            "---\nname: build\ndescription: Local build.\n---\n",
        )?;

        let manager = SkillManager::with_roots(vec![central_skills, local_skills]);
        let skills = manager.discover_skills()?;

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].description, "Local build.");
        Ok(())
    }

    #[test]
    fn skill_entry_point_for_folder_is_skill_md() -> Result<()> {
        let tmp = TempDir::new()?;
        let skills_dir = make_skills_dir(&tmp);
        let skill_dir = skills_dir.join("my-skill");
        fs::create_dir_all(&skill_dir)?;
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: A skill.\n---\n",
        )?;

        let manager = SkillManager::with_roots(vec![skills_dir]);
        let skills = manager.discover_skills()?;

        assert!(skills[0].entry_point().ends_with("my-skill/SKILL.md"));
        Ok(())
    }

    #[test]
    fn skill_entry_point_for_legacy_is_file() -> Result<()> {
        let tmp = TempDir::new()?;
        let skills_dir = make_skills_dir(&tmp);
        let script = skills_dir.join("run.sh");
        fs::write(&script, "#!/bin/bash\n# Run\necho ok")?;
        make_executable(&script);

        let manager = SkillManager::with_roots(vec![skills_dir]);
        let skills = manager.discover_skills()?;

        assert!(skills[0].entry_point().ends_with("run.sh"));
        Ok(())
    }

    #[test]
    fn non_executable_file_ignored() -> Result<()> {
        let tmp = TempDir::new()?;
        let skills_dir = make_skills_dir(&tmp);
        fs::write(skills_dir.join("notes.txt"), "# Not a skill")?;

        let manager = SkillManager::with_roots(vec![skills_dir]);
        let skills = manager.discover_skills()?;
        assert!(skills.is_empty());
        Ok(())
    }

    #[test]
    fn folder_without_skill_md_ignored() -> Result<()> {
        let tmp = TempDir::new()?;
        let skills_dir = make_skills_dir(&tmp);
        fs::create_dir_all(skills_dir.join("not-a-skill"))?;

        let manager = SkillManager::with_roots(vec![skills_dir]);
        let skills = manager.discover_skills()?;
        assert!(skills.is_empty());
        Ok(())
    }

    #[test]
    fn get_skills_summary_empty() {
        let manager = SkillManager::new();
        assert_eq!(manager.get_skills_summary(&[]), "");
    }

    #[test]
    fn get_skills_summary_includes_name_and_description() -> Result<()> {
        let tmp = TempDir::new()?;
        let skills_dir = make_skills_dir(&tmp);
        let skill_dir = skills_dir.join("deploy");
        fs::create_dir_all(&skill_dir)?;
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: deploy\ndescription: Deploy to production.\n---\n",
        )?;

        let manager = SkillManager::with_roots(vec![skills_dir]);
        let skills = manager.discover_skills()?;
        let summary = manager.get_skills_summary(&skills);

        assert!(summary.contains("<available_skills>"));
        assert!(summary.contains("**deploy**"));
        assert!(summary.contains("Deploy to production."));
        assert!(summary.contains("SKILL.md"));
        Ok(())
    }

    #[test]
    fn extract_description_skips_shebang() {
        let content = "#!/bin/bash\n# Deploy app\necho ok";
        assert_eq!(extract_description(content), "Deploy app");
    }

    #[test]
    fn extract_description_missing() {
        let content = "#!/bin/bash\necho ok";
        assert_eq!(extract_description(content), "");
    }
}
