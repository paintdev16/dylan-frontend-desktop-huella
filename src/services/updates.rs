use std::sync::Arc;

use velopack::{Error, UpdateCheck, UpdateInfo, UpdateManager, sources::GithubSource};

pub struct PendingUpdate {
    manager: UpdateManager,
    info: Box<UpdateInfo>,
}

pub enum UpdateState {
    Unavailable,
    Current,
    Busy(&'static str),
    Available(Arc<PendingUpdate>),
    Downloaded(Arc<PendingUpdate>),
    Restarting,
    Failed,
}

impl UpdateState {
    pub fn message(&self) -> String {
        match self {
            Self::Unavailable => "Actualizaciones disponibles en la versión empaquetada.".into(),
            Self::Current => "No hay una versión más reciente.".into(),
            Self::Busy(message) => (*message).into(),
            Self::Available(update) => {
                format!(
                    "Versión {} disponible.",
                    update.info.TargetFullRelease.Version
                )
            }
            Self::Downloaded(update) => format!(
                "Versión {} lista para instalar.",
                update.info.TargetFullRelease.Version
            ),
            Self::Restarting => "Reiniciando para instalar la actualización…".into(),
            Self::Failed => "Falló la actualización. Vuelve a buscar para reintentar.".into(),
        }
    }
}

#[derive(Clone)]
pub enum UpdateAction {
    Check(&'static str),
    Download(Arc<PendingUpdate>),
    Install(Arc<PendingUpdate>),
}

impl UpdateAction {
    pub fn status(&self) -> &'static str {
        match self {
            Self::Check(_) => "Buscando actualizaciones…",
            Self::Download(_) => "Descargando actualización…",
            Self::Install(_) => "Preparando la instalación…",
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
            Self::Download(update) => {
                update.manager.download_updates(&update.info, None)?;
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
