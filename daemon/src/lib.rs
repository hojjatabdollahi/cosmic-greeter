use cosmic_config::CosmicConfigEntry;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub use cosmic_applets_config::time::TimeAppletConfig;
pub use cosmic_bg_config::{Entry as BackgroundEntry, ShaderSource, Source as BackgroundSource};
pub use cosmic_comp_config::{CosmicCompConfig, XkbConfig, ZoomConfig};
pub use cosmic_theme::{Theme, ThemeBuilder};

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct UserData {
    pub uid: u32,
    pub name: String,
    pub full_name: String,
    pub home_dir: PathBuf,
    pub icon_opt: Option<Vec<u8>>,
    pub theme_opt: Option<Theme>,
    pub theme_builder_opt: Option<ThemeBuilder>,
    pub xkb_config_opt: Option<XkbConfig>,
    pub time_applet_config: TimeAppletConfig,
    pub accessibility_zoom: ZoomConfig,
    /// Path to the user's outputs.ron config file
    pub output_config_path: Option<PathBuf>,
    pub shader_source: Option<ShaderSource>,
    pub background_entry: Option<BackgroundEntry>,
}

impl UserData {
    pub fn load_config_as_user(&mut self) {
        self.icon_opt = None;
        self.theme_opt = None;
        self.theme_builder_opt = None;
        self.xkb_config_opt = None;
        self.time_applet_config = Default::default();
        self.shader_source = None;
        self.background_entry = None;

        //TODO: use accountsservice?
        //IMPORTANT: This file is owned by root and safe to read (it won't be a link to /etc/shadow for example)
        // It may not exist if the user uses one of the system icons. In that case, we should read the
        // information in /var/lib/AccountsService/users, and then read the icon path as the user
        let icon_path = Path::new("/var/lib/AccountsService/icons").join(&self.name);
        if icon_path.is_file() {
            match fs::read(&icon_path) {
                Ok(icon_data) => {
                    self.icon_opt = Some(icon_data);
                }
                Err(err) => {
                    tracing::error!("failed to read icon {:?}: {:?}", icon_path, err);
                }
            }
        }

        let mut is_dark = true;
        match cosmic_theme::ThemeMode::config() {
            Ok(helper) => match cosmic_theme::ThemeMode::get_entry(&helper) {
                Ok(theme_mode) => {
                    is_dark = theme_mode.is_dark;
                }
                Err((errs, theme_mode)) => {
                    tracing::error!("failed to load cosmic-theme config: {:?}", errs);
                    is_dark = theme_mode.is_dark;
                }
            },
            Err(err) => {
                tracing::error!("failed to create cosmic-theme mode helper: {:?}", err);
            }
        }

        match if is_dark {
            cosmic_theme::Theme::dark_config()
        } else {
            cosmic_theme::Theme::light_config()
        } {
            Ok(helper) => match cosmic_theme::Theme::get_entry(&helper) {
                Ok(theme) => {
                    self.theme_opt = Some(theme);
                }
                Err((errs, theme)) => {
                    tracing::error!("failed to load cosmic-theme config: {:?}", errs);
                    self.theme_opt = Some(theme);
                }
            },
            Err(err) => {
                tracing::error!("failed to create cosmic-theme config helper: {:?}", err);
            }
        }

        match if is_dark {
            cosmic_theme::ThemeBuilder::dark_config()
        } else {
            cosmic_theme::ThemeBuilder::light_config()
        } {
            Ok(helper) => match cosmic_theme::ThemeBuilder::get_entry(&helper) {
                Ok(theme) => {
                    self.theme_builder_opt = Some(theme);
                }
                Err((errs, theme)) => {
                    tracing::error!("failed to load cosmic-theme builder config: {:?}", errs);
                    self.theme_builder_opt = Some(theme);
                }
            },
            Err(err) => {
                tracing::error!(
                    "failed to create cosmic-theme builder config helper: {:?}",
                    err
                );
            }
        }

        match cosmic_config::Config::new("com.system76.CosmicComp", CosmicCompConfig::VERSION) {
            Ok(config_handler) => {
                match CosmicCompConfig::get_entry(&config_handler) {
                    Ok(config) => {
                        self.xkb_config_opt = Some(config.xkb_config);
                        self.accessibility_zoom = config.accessibility_zoom;
                    }
                    Err((errs, config)) => {
                        tracing::error!("errors loading cosmic-comp config: {:?}", errs);
                        self.xkb_config_opt = Some(config.xkb_config);
                        self.accessibility_zoom = config.accessibility_zoom;
                    }
                };
            }
            Err(err) => {
                tracing::error!("failed to create cosmic-comp config handler: {}", err);
            }
        };

        let xdg = xdg::BaseDirectories::new();
        self.output_config_path = xdg.get_state_home().map(|mut s| {
            s.push("cosmic-comp/outputs.ron");
            s
        });

        match cosmic_config::Config::new("com.system76.CosmicAppletTime", TimeAppletConfig::VERSION)
        {
            Ok(config_handler) => match TimeAppletConfig::get_entry(&config_handler) {
                Ok(config) => {
                    self.time_applet_config = config;
                }
                Err((errs, config)) => {
                    tracing::error!("failed to load time applet config: {:?}", errs);
                    self.time_applet_config = config;
                }
            },
            Err(err) => {
                tracing::error!(
                    "failed to create CosmicAppletTime config handler: {:?}",
                    err
                );
            }
        };

        let home_env = std::env::var("HOME").unwrap_or_else(|_| "UNSET".to_string());
        let xdg_config = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| "UNSET".to_string());
        let expected_config_path = self
            .home_dir
            .join(".config/cosmic/com.system76.CosmicBackground/v1/all");
        tracing::warn!(
            "BG_DAEMON: loading bg config for user {} (HOME={}, XDG_CONFIG_HOME={}, expected_path={})",
            self.name, home_env, xdg_config, expected_config_path.display()
        );
        match cosmic_bg_config::context() {
            Ok(ctx) => {
                // Log the actual config path being used
                let ctx_path = format!("{:?}", ctx);
                tracing::warn!(
                    "BG_DAEMON: bg config context created for user {}, context={:?}",
                    self.name,
                    ctx_path
                );
                match cosmic_bg_config::Config::load(&ctx) {
                    Ok(config) => {
                        let source_type = match &config.default_background.source {
                            cosmic_bg_config::Source::Shader(_) => "Shader",
                            cosmic_bg_config::Source::Path(_) => "Path",
                            cosmic_bg_config::Source::Color(_) => "Color",
                        };
                        tracing::warn!(
                            "BG_DAEMON: bg config loaded for user {}, source type = {}",
                            self.name,
                            source_type
                        );

                        if let cosmic_bg_config::Source::Shader(ref shader) =
                            config.default_background.source
                        {
                            tracing::warn!(
                                "BG_DAEMON: found shader source for user {}: {:?}",
                                self.name,
                                shader
                            );
                            self.shader_source = Some(shader.clone());
                        }

                        tracing::warn!(
                            "BG_DAEMON: storing background entry for user {}: {:?}",
                            self.name,
                            config.default_background
                        );
                        self.background_entry = Some(config.default_background);
                    }
                    Err(err) => {
                        tracing::warn!(
                            "BG_DAEMON: failed to load bg config for user {}: {:?}",
                            self.name,
                            err
                        );
                    }
                }
            }
            Err(err) => {
                tracing::warn!(
                    "BG_DAEMON: failed to create bg config context for user {}: {:?}",
                    self.name,
                    err
                );
            }
        }
    }
}

impl From<pwd::Passwd> for UserData {
    fn from(user: pwd::Passwd) -> Self {
        let mut full_name = user
            .gecos
            .as_ref()
            .and_then(|gecos| gecos.split(',').next())
            .map(|x| x.to_string())
            .unwrap_or_default();
        if full_name.is_empty() {
            full_name = user.name.clone();
        }
        Self {
            uid: user.uid,
            name: user.name.clone(),
            full_name,
            home_dir: PathBuf::from(&user.dir),
            ..Default::default()
        }
    }
}
