use crate::models::{Args, Commands};
use anyhow::Result;
use clap::Parser;

/// Main CLI entry point
pub fn run() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Recipe {
            file,
            tag,
            upgrade,
            profile,
            lock,
        } => {
            crate::recipe::process_recipe(
                &file,
                tag.as_deref(),
                upgrade,
                profile.as_deref(),
                lock,
            )?;
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
            // TODO: Implement run
            crate::runner::run_package()?;
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
            // TODO: Implement install
            crate::install::executable::install_package()?;
        }
        Commands::Shim { target_executable } => {
            println!("Shim command: target_executable={target_executable}");
            // TODO: Implement shim creation
            crate::install::shim::create_shim(&target_executable)?;
        }
    }

    Ok(())
}
