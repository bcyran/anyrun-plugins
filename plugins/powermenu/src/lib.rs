#![allow(clippy::needless_pass_by_value, clippy::wildcard_imports)]
use core::str;
use std::{
    fs,
    io::Error,
    process::{Command, Output},
};

use abi_stable::std_types::{ROption, RString, RVec};
use anyrun_plugin::*;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use ron::Result;
use serde::Deserialize;

#[derive(Deserialize, Default)]
struct PowerActionConfig {
    command: String,
    confirm: bool,
}

#[derive(Deserialize)]
struct Config {
    #[serde(default = "Config::default_lock_config")]
    lock: PowerActionConfig,
    #[serde(default = "Config::default_logout_config")]
    logout: PowerActionConfig,
    #[serde(default = "Config::default_poweroff_config")]
    poweroff: PowerActionConfig,
    #[serde(default = "Config::default_reboot_config")]
    reboot: PowerActionConfig,
    #[serde(default = "Config::default_suspend_config")]
    suspend: PowerActionConfig,
    #[serde(default = "Config::default_hibernate_config")]
    hibernate: PowerActionConfig,
}

impl Config {
    fn default_lock_config() -> PowerActionConfig {
        PowerActionConfig {
            command: String::from("loginctl lock-session"),
            confirm: false,
        }
    }

    fn default_logout_config() -> PowerActionConfig {
        PowerActionConfig {
            command: String::from("loginctl terminate-user $USER"),
            confirm: true,
        }
    }

    fn default_poweroff_config() -> PowerActionConfig {
        PowerActionConfig {
            command: String::from("systemctl -i poweroff"),
            confirm: true,
        }
    }

    fn default_reboot_config() -> PowerActionConfig {
        PowerActionConfig {
            command: String::from("systemctl -i reboot"),
            confirm: true,
        }
    }

    fn default_suspend_config() -> PowerActionConfig {
        PowerActionConfig {
            command: String::from("systemctl -i suspend"),
            confirm: false,
        }
    }

    fn default_hibernate_config() -> PowerActionConfig {
        PowerActionConfig {
            command: String::from("systemctl -i hibernate"),
            confirm: false,
        }
    }

    const fn get_action_config(&self, action: PowerAction) -> &PowerActionConfig {
        match action {
            PowerAction::Lock => &self.lock,
            PowerAction::Logout => &self.logout,
            PowerAction::Poweroff => &self.poweroff,
            PowerAction::Reboot => &self.reboot,
            PowerAction::Suspend => &self.suspend,
            PowerAction::Hibernate => &self.hibernate,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lock: Self::default_lock_config(),
            logout: Self::default_logout_config(),
            poweroff: Self::default_poweroff_config(),
            reboot: Self::default_reboot_config(),
            suspend: Self::default_suspend_config(),
            hibernate: Self::default_hibernate_config(),
        }
    }
}

#[derive(Clone, Copy, IntoPrimitive, TryFromPrimitive)]
#[repr(u64)]
enum PowerAction {
    Lock,
    Logout,
    Poweroff,
    Reboot,
    Suspend,
    Hibernate,
}

impl PowerAction {
    const VALUES: [Self; 6] = [
        Self::Lock,
        Self::Logout,
        Self::Poweroff,
        Self::Reboot,
        Self::Suspend,
        Self::Hibernate,
    ];

    const fn get_title(&self) -> &str {
        match self {
            Self::Lock => "Lock",
            Self::Logout => "Log out",
            Self::Poweroff => "Power off",
            Self::Reboot => "Reboot",
            Self::Suspend => "Suspend",
            Self::Hibernate => "Hibernate",
        }
    }
    const fn get_description(&self) -> &str {
        match self {
            Self::Lock => "Lock the session screen",
            Self::Logout => "Terminate the session",
            Self::Poweroff => "Shut down the system",
            Self::Reboot => "Restart the system",
            Self::Suspend => "Suspend the system to RAM",
            Self::Hibernate => "Suspend the system to disk",
        }
    }

    const fn get_icon_name(&self) -> &str {
        match self {
            Self::Lock => "system-lock-screen",
            Self::Logout => "system-log-out",
            Self::Poweroff => "system-shutdown",
            Self::Reboot => "system-reboot",
            Self::Suspend => "system-suspend",
            Self::Hibernate => "system-suspend-hibernate",
        }
    }

    fn as_match(self) -> Match {
        Match {
            title: self.get_title().into(),
            icon: ROption::RSome(self.get_icon_name().into()),
            use_pango: false,
            description: ROption::RSome(self.get_description().into()),
            id: ROption::RSome(self.into()),
        }
    }

    fn get_fuzzy_matching_values(phrase: &str) -> impl Iterator<Item = Self> {
        let fuzzy_matcher = SkimMatcherV2::default().ignore_case();
        let mut matches_with_scores = Self::VALUES
            .into_iter()
            .filter_map(|action| {
                action
                    .get_fuzzy_score(&fuzzy_matcher, phrase)
                    .map(|score| (action, score))
            })
            .collect::<Vec<_>>();
        matches_with_scores.sort_by_key(|(_action, score)| *score);
        matches_with_scores
            .into_iter()
            .map(|(action, _score)| action)
    }

    fn get_fuzzy_score(self, matcher: &impl FuzzyMatcher, phrase: &str) -> Option<i64> {
        matcher
            .fuzzy_match(self.get_title(), phrase)
            .max(matcher.fuzzy_match(self.get_description(), phrase))
    }
}

#[derive(PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u64)]
enum ConfirmAction {
    Confirm,
    Cancel,
}

impl ConfirmAction {
    fn is_confirmed(&self) -> bool {
        *self == Self::Confirm
    }
}

pub struct State {
    config: Config,
    pending_action: Option<PowerAction>,
    error_message: Option<String>,
}

#[init]
fn init(config_dir: RString) -> State {
    let config = fs::read_to_string(format!("{config_dir}/powermenu.ron")).map_or_else(
        |_err| Config::default(),
        |content| ron::from_str(&content).unwrap_or_default(),
    );

    State {
        config,
        pending_action: None,
        error_message: None,
    }
}

#[info]
fn info() -> PluginInfo {
    PluginInfo {
        name: "Power menu".into(),
        icon: "computer".into(),
    }
}

#[get_matches]
fn get_matches(input: RString, state: &State) -> RVec<Match> {
    if let Some(ref error_message) = state.error_message {
        get_error_matches(error_message)
    } else if let Some(pending_action) = state.pending_action {
        get_confirm_matches(pending_action)
    } else {
        PowerAction::get_fuzzy_matching_values(&input)
            .map(PowerAction::as_match)
            .collect()
    }
    .into()
}

fn get_confirm_matches(action_to_confirm: PowerAction) -> Vec<Match> {
    vec![
        Match {
            title: action_to_confirm.get_title().into(),
            icon: ROption::RSome("go-next".into()),
            use_pango: false,
            description: ROption::RSome("Proceed with the selected action".into()),
            id: ROption::RSome(ConfirmAction::Confirm.into()),
        },
        Match {
            title: "Cancel".into(),
            icon: ROption::RSome("go-previous".into()),
            use_pango: false,
            description: ROption::RSome("Abort the selected action".into()),
            id: ROption::RSome(ConfirmAction::Cancel.into()),
        },
    ]
}

fn get_error_matches(error_message: &str) -> Vec<Match> {
    vec![Match {
        title: "ERROR!".into(),
        icon: ROption::RSome("dialog-error".into()),
        use_pango: false,
        description: ROption::RSome(error_message.into()),
        id: ROption::RSome(ConfirmAction::Confirm.into()),
    }]
}

#[handler]
fn handler(selection: Match, state: &mut State) -> HandleResult {
    if state.error_message.is_some() {
        return HandleResult::Close;
    }

    let power_action_config = if let Some(ref pending_action) = state.pending_action {
        let confirm_action = ConfirmAction::try_from(selection.id.unwrap()).unwrap();

        if !confirm_action.is_confirmed() {
            state.pending_action = None;
            return HandleResult::Refresh(false);
        }

        state.config.get_action_config(*pending_action)
    } else {
        let power_action = PowerAction::try_from(selection.id.unwrap()).unwrap();
        let power_action_config = state.config.get_action_config(power_action);

        if power_action_config.confirm {
            state.pending_action = Some(power_action);
            return HandleResult::Refresh(true);
        };

        power_action_config
    };

    let action_result = execute_power_action(power_action_config);
    let error_message = get_error_message(action_result);
    if error_message.is_some() {
        state.error_message = error_message;
        return HandleResult::Refresh(true);
    }

    HandleResult::Close
}

fn execute_power_action(action: &PowerActionConfig) -> Result<Output, std::io::Error> {
    Command::new("/usr/bin/env")
        .arg("sh")
        .arg("-c")
        .arg(&action.command)
        .output()
}

fn get_error_message(command_result: Result<Output, Error>) -> Option<String> {
    match command_result {
        Err(err) => Some(format!("Could not run command: {err}")),
        Ok(output) if !output.status.success() => Some(format!(
            "{}, stderr: {}",
            output.status,
            String::from_utf8_lossy(output.stderr.as_ref())
        )),
        Ok(_) => None,
    }
}
