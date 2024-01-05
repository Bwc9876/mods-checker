use clap::Parser;
use cli::CheckerSubcommand;
use octocrab::{models::repos::Release, repos::RepoHandler, Octocrab};
use owmods_core::{
    config::Config,
    constants::{DEFAULT_ALERT_URL, DEFAULT_DB_URL},
    db::{LocalDatabase, RemoteDatabase},
    download::{install_mod_from_url, install_mod_from_zip},
    mods::local::{LocalMod, ModManifest},
};
use tempfile::TempDir;
use thiserror::Error;

mod cli;

#[derive(Error, Debug)]
pub enum CheckerError {
    #[error(
        "This unique name appears to be in use by another mod ({0}), please choose a different one"
    )]
    UniqueNameInUse(String),
    #[error("This mod's repo doesn't appear to exist")]
    MissingRepo,
    #[error("This mod appears to be missing a release, did you forget to publish it?")]
    MissingRelease,
    #[error("This mod has a release, but it's missing the mod asset, make sure you've uploaded a ZIP file")]
    MissingModAsset,
    #[error("This mod failed to install when testing it: {0}")]
    FailedToInstall(String),
    #[error(
        "The unique name of this mod is not what was expected, expected {expected}, got {actual}"
    )]
    UnexpectedUniqueName { expected: String, actual: String },
    #[error("The version of this mod's manifest does not match the tag of the release, expected {expected}, got {actual}")]
    UnexpectedVersion { expected: String, actual: String },
    #[error("This mod's manifest doesn't define a DLL file")]
    MissingDLL,
    #[error("This mod's manifest defines a DLL file that doesn't exist: {0}")]
    InvalidDLL(String),
    #[error("This mod depends on another mod that seemingly doesn't exist: {0}")]
    MissingDependency(String),
}

#[derive(Error, Debug)]
pub enum CheckerWarning {
    #[error("This mod's repo doesn't have a description, the description is used on the manager and website to describe your mod")]
    MissingDescription,
    #[error("This mod's repo doesn't have a README, the README is used on the website to describe your mod")]
    MissingReadme,
}

type Result<T = (), E = CheckerError> = std::result::Result<T, E>;

async fn get_latest_release(repo: &RepoHandler<'_>) -> Result<Release> {
    println!("Getting Latest Release...");
    let release = repo
        .releases()
        .get_latest()
        .await
        .map_err(|_| CheckerError::MissingRelease)?;
    Ok(release)
}

fn compare_unique_names(local_mod: &ModManifest, expected: &str) -> Result {
    println!("Checking Unique Names...");
    let actual = local_mod.unique_name.as_str();
    if actual != expected {
        Err(CheckerError::UnexpectedUniqueName {
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    } else {
        Ok(())
    }
}

fn compare_tag_and_version(release: &Release, local_mod: &ModManifest) -> Result {
    println!("Checking Versions...");
    let expected = release.tag_name.as_str().trim_start_matches('v');
    let actual = local_mod.version.as_str().trim_start_matches('v');
    if actual != expected {
        Err(CheckerError::UnexpectedVersion {
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    } else {
        Ok(())
    }
}

fn check_dependencies(local_mod: &ModManifest, remote_db: &RemoteDatabase) -> Result {
    println!("Checking Dependencies...");
    if let Some(dependencies) = &local_mod.dependencies {
        for dependency in dependencies {
            if !remote_db.mods.contains_key(dependency) {
                return Err(CheckerError::MissingDependency(dependency.clone()));
            }
        }
    }
    Ok(())
}

async fn check_for_description_and_readme(
    octo: &Octocrab,
    owner: &str,
    repo_name: &str,
    repo: &RepoHandler<'_>,
    warnings: &mut Vec<CheckerWarning>,
) {
    println!("Checking For Description...");
    let m_repo: octocrab::models::Repository = octo
        .get(format!("/repos/{owner}/{repo_name}"), None::<&()>)
        .await
        .unwrap();
    if m_repo
        .description
        .as_ref()
        .map(String::is_empty)
        .unwrap_or(true)
    {
        warnings.push(CheckerWarning::MissingDescription);
    }
    println!("Checking For Readme...");
    if repo.get_readme().send().await.is_err() {
        warnings.push(CheckerWarning::MissingReadme);
    }
}

async fn check_mod(
    sub: CheckerSubcommand,
    expected_unique_name: Option<&str>,
    check_remote: bool,
) -> Result<Vec<CheckerWarning>> {
    let mut warnings = vec![];

    let working_dir = TempDir::new().unwrap();
    let path = working_dir.path();
    let config = Config {
        owml_path: path.to_str().unwrap().to_string(),
        database_url: DEFAULT_DB_URL.to_string(),
        alert_url: DEFAULT_ALERT_URL.to_string(),
        viewed_alerts: vec![],
        path: path.join("config.json"),
    };

    println!("Initializing RemoteDatabase...");

    let remote_db = RemoteDatabase::fetch(&config.database_url).await.unwrap();
    if check_remote {
        if let Some(unique_name) = expected_unique_name {
            println!("Checking Unique Names...");
            if let Some(remote_mod) = remote_db.get_mod(unique_name) {
                return Err(CheckerError::UniqueNameInUse(remote_mod.name.clone()));
            }
        }
    }

    println!("Initializing LocalDatabase...");

    let local_db = LocalDatabase::fetch(&config.owml_path).unwrap();

    let local_mod = install_mod(sub, &config, local_db, &mut warnings).await?;

    let manifest = local_mod.manifest;

    if let Some(unique_name) = expected_unique_name {
        compare_unique_names(&manifest, unique_name)?;
    } else if check_remote {
        println!("Checking Unique Names...");
        if let Some(remote_mod) = remote_db.get_mod(&manifest.unique_name) {
            return Err(CheckerError::UniqueNameInUse(remote_mod.name.clone()));
        }
    }

    check_dependencies(&manifest, &remote_db)?;

    println!("Passed All Checks! Cleaning Up...");

    working_dir.close().unwrap();

    Ok(warnings)
}

async fn install_mod(
    sub: CheckerSubcommand,
    config: &Config,
    local_db: LocalDatabase,
    warnings: &mut Vec<CheckerWarning>,
) -> Result<LocalMod> {
    match sub {
        CheckerSubcommand::Repo { repo } => {
            println!("Fetching Repo...");

            let octo = octocrab::instance();
            let (owner, repo_name) = repo.split_once('/').ok_or(CheckerError::MissingRelease)?;
            let repo = octo.repos(owner, repo_name);

            if repo.get().await.is_err() {
                return Err(CheckerError::MissingRepo);
            }

            let release = get_latest_release(&repo).await?;
            let asset = release
                .assets
                .iter()
                .find(|asset| asset.name.ends_with(".zip"))
                .ok_or(CheckerError::MissingModAsset)?;
            let download_url = asset.browser_download_url.to_string();

            check_for_description_and_readme(&octo, owner, repo_name, &repo, warnings).await;

            println!("Installing Mod...");

            let local_mod = install_mod_from_url(&download_url, None, config, &local_db)
                .await
                .map_err(|e| CheckerError::FailedToInstall(e.to_string()))?;

            compare_tag_and_version(&release, &local_mod.manifest)?;

            Ok(local_mod)
        }
        CheckerSubcommand::Url { url } => {
            println!("Installing Mod...");
            let local_mod = install_mod_from_url(&url, None, config, &local_db)
                .await
                .map_err(|e| CheckerError::FailedToInstall(e.to_string()))?;
            Ok(local_mod)
        }
        CheckerSubcommand::File { file } => {
            println!("Installing Mod...");
            let local_mod = install_mod_from_zip(&file, config, &local_db)
                .map_err(|e| CheckerError::FailedToInstall(e.to_string()))?;
            Ok(local_mod)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), CheckerError> {
    let cli = cli::CheckerCli::parse();

    let expected_unique_name = cli.expected_unique_name.as_deref();

    let res = check_mod(cli.command, expected_unique_name, !cli.skip_exists).await;

    match res {
        Ok(warnings) => {
            for warning in warnings {
                eprintln!("Warning: {}", warning);
            }
        }
        Err(e) => eprintln!("Mod is invalid: {}", e),
    }

    Ok(())
}
