use crate::models::{Args, Commands, RecipeOptions};
use anyhow::Result;
use clap::Parser;

/// Main CLI entry point
pub fn run() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Recipe {
            file,
            tags,
            exclude,
            upgrade,
            profile,
            lock,
            var_overrides,
            dry,
        } => {
            let opts = RecipeOptions {
                tags: &tags,
                exclude: &exclude,
                upgrade,
                profile: profile.as_deref(),
                lock,
                var_overrides: &var_overrides,
                dry,
            };
            crate::recipe::process_recipe(&file, &opts)?;
        }
        Commands::Github {
            repo,
            binary,
            save_as,
            tag,
            unzip_to,
            files,
        } => {
            crate::download::github::fetch_github_release(
                &repo,
                binary.as_deref(),
                save_as.as_deref(),
                tag.as_deref(),
                unzip_to.as_deref(),
                files.as_deref(),
            )?;
        }
        Commands::Fetch {
            url,
            save_as,
            unzip_to,
            files,
            profile,
        } => {
            println!(
                "Fetch command: url={url}, save_as={save_as:?}, unzip_to={unzip_to:?}, files={files:?}, profile={profile:?}"
            );
            // Call the fetch function with the parameters
            crate::runner::fetch_direct_url(
                &url,
                save_as.as_deref(),
                unzip_to.as_deref(),
                files.as_deref(),
                profile.as_deref(),
            )?;
        }
        Commands::Run {
            source,
            binary,
            tag,
            files,
            profile,
            executable,
            args,
        } => {
            println!(
                "Run command: source={source}, binary={binary:?}, tag={tag:?}, files={files:?}, profile={profile:?}, executable={executable:?}, args={args:?}"
            );
            crate::runner::run_package(
                &source,
                binary.as_deref(),
                tag.as_deref(),
                files.as_deref(),
                profile.as_deref(),
                executable.as_deref(),
                &args,
            )?;
        }
        Commands::Install {
            source,
            binary,
            tag,
            files,
            profile,
            executable,
            no_shim,
        } => {
            println!(
                "Install command: source={source}, binary={binary:?}, tag={tag:?}, files={files:?}, profile={profile:?}, executable={executable:?}, no_shim={no_shim}"
            );
            crate::install::executable::install_package(
                &source,
                binary.as_deref(),
                tag.as_deref(),
                files.as_deref(),
                profile.as_deref(),
                executable.as_deref(),
                no_shim,
            )?;
        }
        Commands::Shim { target_executable } => {
            println!("Shim command: target_executable={target_executable}");
            // TODO: Implement shim creation
            crate::install::shim::create_shim(&target_executable)?;
        }
        Commands::Update => {
            crate::update::self_update()?;
        }
    }

    Ok(())
}
