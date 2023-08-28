mod dryrun;
mod strings;
pub mod operation;
mod project_version;

use std::path::{Path, PathBuf};
use git2::{Repository};
use mockall_double::double;
use crate::config::{Config, RepoConfig};
use crate::error::SheepError;
use crate::project::operation::Operation;
use crate::repo::clone::GitCloner;
use crate::repo::open::{GitOpener};
use crate::repo::path;
use crate::repo::remote::GitRemotes;

#[double]
use crate::project::project_version::ProjectVersion;
use crate::project::strings::ProjectStrings;
use crate::repo::branch::GitBranches;
use crate::repo::commit::GitCommits;
use crate::repo::tag::GitTags;

pub struct Project {
    config: Config,
    repo: Repository,
    is_dry_run_project: bool,
}

impl Project {
    pub fn new_local_project<P: AsRef<Path>>(path: P) -> Result<Project, SheepError> {
        let repo = GitOpener::new().open(path)?;
        let config = Config::default();
        let project = Project {
            config,
            repo,
            is_dry_run_project: false,
        };
        Ok(project)
    }

    pub fn new_remote_project<P: AsRef<Path>>(url: &str, directory: P) -> Result<Project, SheepError> {
        let repo_path = path::repo_path(url, directory)?;
        let repo = GitCloner::new().clone(url, repo_path)?;
        let config = Config::default();
        let project = Project {
            config,
            repo,
            is_dry_run_project: false,
        };
        Ok(project)
    }

    pub fn new_dry_run_project<P: AsRef<Path>>(path: P) -> Result<Project, SheepError> {
        let remotes = GitRemotes::new();
        let local_project = Project::new_local_project(path)?;
        let remote_url = remotes.remote_url(&local_project.repo, "origin")?;
        let directory = dryrun::directory()?;

        let remote_project = Project::new_remote_project(&remote_url, directory)?;
        let dry_run_project = Project {
            config: local_project.config,
            is_dry_run_project: true,
            repo: remote_project.repo,
        };
        Ok(dry_run_project)
    }

    pub fn update(&self, operation: Operation) -> Result<ProjectUpdateInfo, SheepError> {
        let repo_config = &self.config.repository;
        let project_version = ProjectVersion::new(&self);
        let version_update = operation.version_update(&project_version);
        let project_strings = ProjectStrings::new(&self.config, &version_update);

        Self::update_repo(&self.repo, repo_config, &project_strings)?;
        // Process subprojects if there are any

        // Return project info
        let repo_path = self.repo.path();
        Ok(ProjectUpdateInfo::new(repo_path))
    }

    fn update_repo(
        repo: &Repository,
        repo_config: &RepoConfig,
        project_strings: &ProjectStrings) -> Result<(), SheepError> {
        // Create branch if enabled in configuration
        if repo_config.enable_branch {
            let branches = GitBranches::new();
            branches.create_branch(repo, &project_strings.branch_name)?;
            branches.checkout_branch(repo, &project_strings.branch_name)?;
        }
        // Create commit if enabled in configuration
        if repo_config.enable_commit {
            let commits = GitCommits::new();
            commits.commit(repo, vec![], &project_strings.commit_message)?;
        }
        // Create tag if enabled in configuration
        if repo_config.enable_tag {
            let tags = GitTags::new();
            tags.create_tag(repo, &project_strings.tag_name, None)?;
        }
        // Push if enabled in configuration
        if repo_config.enable_push {
            let remotes = GitRemotes::new();
            if repo_config.enable_branch {
                remotes.push_branch(repo,
                                    &project_strings.branch_name,
                                    &project_strings.remote_name)?;
            }
            if repo_config.enable_tag {
                remotes.push_tag(repo,
                                 &project_strings.tag_name,
                                 &project_strings.remote_name)?;
            }
        }
        Ok(())
    }
}

pub struct ProjectUpdateInfo {
    pub repo_path: PathBuf,
}

impl ProjectUpdateInfo {
    fn new(repo_path: &Path) -> ProjectUpdateInfo {
        ProjectUpdateInfo {
            repo_path: repo_path.to_path_buf()
        }
    }
}