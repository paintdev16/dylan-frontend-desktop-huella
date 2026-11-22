use std::sync::{Arc, mpsc::Sender};

use velopack::{Error, UpdateCheck, UpdateInfo, UpdateManager, sources::GithubSource};

pub struct PendingUpdate {
    manager: UpdateManager,
    info: Box<UpdateInfo>,
}

impl PendingUpdate {
    pub fn version(&self) -> &str {
        &self.info.TargetFullRelease.Version
    }
}

pub enum UpdateState {
    Unavailable,
    Current,
    Checking,
    Available(Arc<PendingUpdate>),
    Downloading(Arc<PendingUpdate>),
    Downloaded(Arc<PendingUpdate>),
    Installing(Arc<PendingUpdate>),
    Restarting,
    Failed,
}

pub enum UpdateAction {
    Check(&'static str),
    Download(Arc<PendingUpdate>, Sender<i16>),
    Install(Arc<PendingUpdate>),
}

impl UpdateAction {
    pub fn pending_state(&self) -> UpdateState {
        match self {
            Self::Check(_) => UpdateState::Checking,
            Self::Download(update, _) => UpdateState::Downloading(update.clone()),
            Self::Install(update) => UpdateState::Installing(update.clone()),
        }
    }

    pub fn run(self) -> Result<UpdateState, Error> {
        match self {
            Self::Check(repository) => {
                let source =
                    GithubSource::new(&format!("https://github.com/{repository}"), None, false);
                let manager = match UpdateManager::new(source, None, None) {
                    Ok(manager) => manager,
                    Err(Error::NotInstalled(_)) => return Ok(UpdateState::Unavailable),
                    Err(error) => return Err(error),
                };
                match manager.check_for_updates()? {
                    UpdateCheck::UpdateAvailable(info) => {
                        Ok(UpdateState::Available(Arc::new(PendingUpdate {
                            manager,
                            info,
                        })))
                    }
                    UpdateCheck::NoUpdateAvailable | UpdateCheck::RemoteIsEmpty => {
                        Ok(UpdateState::Current)
                    }
                }
            }
            Self::Download(update, progress) => {
                update
                    .manager
                    .download_updates(&update.info, Some(progress))?;
                Ok(UpdateState::Downloaded(update))
            }
            Self::Install(update) => {
                update.manager.wait_exit_then_apply_updates(
                    update.info.as_ref(),
                    false,
                    true,
                    std::iter::empty::<&str>(),
                )?;
                Ok(UpdateState::Restarting)
            }
        }
    }
}
