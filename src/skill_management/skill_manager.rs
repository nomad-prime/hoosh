use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

impl Skill {
    fn from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .context(format!("Failed to read skill file: {}", path.display()))?;

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
        })
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

pub struct SkillManager;

impl SkillManager {
    pub fn new() -> Self {
        SkillManager
    }

    pub fn discover_skills(&self, project_root: &Path) -> Result<Vec<Skill>> {
        let skills_dir = project_root.join(".hoosh").join("skills");

        if !skills_dir.exists() {
            return Ok(Vec::new());
        }

        let mut skills = Vec::new();

        for entry in WalkDir::new(&skills_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
        {
            let path = entry.path();

            if self.is_skill_file(path) {
                match Skill::from_file(path) {
                    Ok(skill) => skills.push(skill),
                    Err(e) => {
                        eprintln!("Warning: Failed to load skill {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(skills)
    }

    fn is_skill_file(&self, path: &Path) -> bool {
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
            true
        }
    }

    pub fn get_skills_summary(&self, skills: &[Skill]) -> String {
        if skills.is_empty() {
            return String::new();
        }

        let mut summary = "<available_skills>\n".to_string();

        summary.push_str("Project-specific utilities in <PWD>/.hoosh/skills/. ");
        summary.push_str("ALWAYS check these skills first before using bash commands. ");
        summary.push_str("Read the skill file to understand what it does, then execute it.\n\n");

        for skill in skills {
            summary.push_str(&format!("- **{}**: ", skill.name));
            summary.push_str(&format!("path: {} \n", skill.path.display()));
            if !skill.description.is_empty() {
                summary.push_str(&skill.description);
            } else {
                summary.push_str("(no description)");
            }
            summary.push('\n');
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

    #[test]
    fn test_skill_manager_creates() {
        let skill_manager = SkillManager::new();
        // On non-Unix, this will always return true for non-existent paths
        // On Unix, this will return false for non-existent paths
        let _ = skill_manager.is_skill_file(Path::new("script.sh"));
    }

    #[test]
    fn test_is_skill_file_recognizes_extensions() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let skill_manager = SkillManager::new();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let extensions = vec!["sh", "bash", "py", "js", "ts"];
            for ext in extensions {
                let path = temp_dir.path().join(format!("script.{}", ext));
                fs::write(&path, "#!/bin/bash\necho test")?;

                let mut perms = fs::metadata(&path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&path, perms)?;

                assert!(
                    skill_manager.is_skill_file(&path),
                    "File script.{} should be recognized as executable skill",
                    ext
                );
            }

            let non_exec_path = temp_dir.path().join("script.txt");
            fs::write(&non_exec_path, "echo test")?;
            assert!(
                !skill_manager.is_skill_file(&non_exec_path),
                "Non-executable file should not be recognized"
            );
        }

        #[cfg(not(unix))]
        {
            // On non-Unix, just verify the function doesn't panic
            let _ = skill_manager.is_skill_file(temp_dir.path());
        }

        Ok(())
    }

    #[test]
    fn test_discover_skills_empty_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let skill_manager = SkillManager::new();
        let skills = skill_manager.discover_skills(temp_dir.path())?;
        assert_eq!(skills.len(), 0);
        Ok(())
    }

    #[test]
    fn test_discover_skills_with_script() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let skills_dir = temp_dir.path().join(".hoosh").join("skills");
        fs::create_dir_all(&skills_dir)?;

        let script_path = skills_dir.join("test_skill.sh");
        fs::write(
            &script_path,
            "#!/bin/bash\n# This is a test skill\necho 'Hello'",
        )?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms)?;
        }

        let skill_manager = SkillManager::new();
        let skills = skill_manager.discover_skills(temp_dir.path())?;

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "test_skill");
        assert_eq!(skills[0].description, "This is a test skill");
        Ok(())
    }

    #[test]
    fn test_extract_description_with_comment() {
        let content = "#!/bin/bash\n# Deploy to production\necho 'deploying'";
        let desc = extract_description(content);
        assert_eq!(desc, "Deploy to production");
    }

    #[test]
    fn test_extract_description_without_comment() {
        let content = "#!/bin/bash\necho 'no description'";
        let desc = extract_description(content);
        assert_eq!(desc, "");
    }

    #[test]
    fn test_discover_multiple_skills() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let skills_dir = temp_dir.path().join(".hoosh").join("skills");
        fs::create_dir_all(&skills_dir)?;

        let skill1_path = skills_dir.join("skill1.sh");
        fs::write(&skill1_path, "#!/bin/bash\n# First skill\necho 'skill1'")?;

        let skill2_path = skills_dir.join("skill2.py");
        fs::write(
            &skill2_path,
            "#!/usr/bin/env python3\n# Second skill\nprint('skill2')",
        )?;

        fs::write(
            skills_dir.join("skill3.txt"),
            "# Not a skill (wrong extension)",
        )?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&skill1_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&skill1_path, perms)?;

            let mut perms = fs::metadata(&skill2_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&skill2_path, perms)?;
        }

        let skill_manager = SkillManager::new();
        let skills = skill_manager.discover_skills(temp_dir.path())?;

        assert_eq!(skills.len(), 2);
        assert!(skills.iter().any(|s| s.name == "skill1"));
        assert!(skills.iter().any(|s| s.name == "skill2"));
        Ok(())
    }

    #[test]
    fn test_get_skills_summary() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let skills_dir = temp_dir.path().join(".hoosh").join("skills");
        fs::create_dir_all(&skills_dir)?;

        let deploy_path = skills_dir.join("deploy.sh");
        fs::write(
            &deploy_path,
            "#!/bin/bash\n# Deploy application\necho 'deploying'",
        )?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&deploy_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&deploy_path, perms)?;
        }

        let skill_manager = SkillManager::new();
        let skills = skill_manager.discover_skills(temp_dir.path())?;
        let summary = skill_manager.get_skills_summary(&skills);

        assert!(summary.contains("<available_skills>"));
        assert!(summary.contains("deploy"));
        assert!(summary.contains("Deploy application"));
        Ok(())
    }

    #[test]
    fn test_get_skills_summary_empty() {
        let skill_manager = SkillManager::new();
        let summary = skill_manager.get_skills_summary(&[]);
        assert_eq!(summary, "");
    }

    #[test]
    fn test_skill_no_description() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let skills_dir = temp_dir.path().join(".hoosh").join("skills");
        fs::create_dir_all(&skills_dir)?;

        let nodesc_path = skills_dir.join("nodesc.sh");
        fs::write(&nodesc_path, "#!/bin/bash\necho 'no description'")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&nodesc_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&nodesc_path, perms)?;
        }

        let skill_manager = SkillManager::new();
        let skills = skill_manager.discover_skills(temp_dir.path())?;

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].description, "");
        Ok(())
    }

    #[test]
    fn test_is_skill_file_checks_executable() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let skill_path = temp_dir.path().join("skill.sh");

        // Not executable yet
        fs::write(&skill_path, "#!/bin/bash\necho test")?;
        let skill_manager = SkillManager::new();
        assert!(!skill_manager.is_skill_file(&skill_path));

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&skill_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&skill_path, perms)?;
            assert!(skill_manager.is_skill_file(&skill_path));
        }

        Ok(())
    }
}
