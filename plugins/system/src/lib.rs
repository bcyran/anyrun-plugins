#![allow(clippy::needless_pass_by_value, clippy::wildcard_imports)]
use std::fs;

use abi_stable::std_types::{ROption, RString, RVec};
use anyrun_plugin::*;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::Deserialize;

macro_rules! string_vec {
    ($($x:expr),*) => (vec![$($x.to_string()),*]);
}

#[derive(Deserialize, Default)]
struct PowerActionConfig {
    command: Vec<String>,
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
            command: string_vec!["loginctl", "lock-session"],
            confirm: false,
        }
    }

    fn default_logout_config() -> PowerActionConfig {
        PowerActionConfig {
            command: string_vec!["loginctl", "terminate-session", "$USER"],
            confirm: true,
        }
    }

    fn default_poweroff_config() -> PowerActionConfig {
        PowerActionConfig {
            command: string_vec!["systemctl", "-i", "poweroff"],
            confirm: true,
        }
    }

    fn default_reboot_config() -> PowerActionConfig {
        PowerActionConfig {
            command: string_vec!["systemctl", "-i", "reboot"],
            confirm: true,
        }
    }

    fn default_suspend_config() -> PowerActionConfig {
        PowerActionConfig {
            command: string_vec!["systemctl", "-i", "suspend"],
            confirm: false,
        }
    }

    fn default_hibernate_config() -> PowerActionConfig {
        PowerActionConfig {
            command: string_vec!["systemctl", "-i", "hibernate"],
            confirm: false,
        }
    }

    const fn get_action_config(&self, action: &PowerAction) -> &PowerActionConfig {
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

#[derive(IntoPrimitive, TryFromPrimitive)]
#[repr(u64)]
enum PowerAction {
    Lock,
    Logout,
    Poweroff,
    Reboot,
    Suspend,
    Hibernate,
}

#[derive(PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u64)]
enum ConfirmAction {
    Confirm,
    Cancel,
}

pub struct State {
    config: Config,
    pending_action: Option<PowerAction>,
}

#[init]
fn init(config_dir: RString) -> State {
    let config = fs::read_to_string(format!("{config_dir}/system.ron")).map_or_else(
        |_err| Config::default(),
        |content| ron::from_str(&content).unwrap_or_default(),
    );

    State {
        config,
        pending_action: None,
    }
}

#[info]
fn info() -> PluginInfo {
    PluginInfo {
        name: "System actions".into(),
        icon: "computer".into(),
    }
}

#[get_matches]
fn get_matches(input: RString, state: &State) -> RVec<Match> {
    let matches = if state.pending_action.is_some() {
        get_confirm_matches()
    } else {
        get_fuzzy_matches(get_action_matches().into_iter(), &input)
    };

    matches.into()
}

fn get_fuzzy_matches(matches: impl Iterator<Item = Match>, phrase: &str) -> Vec<Match> {
    let fuzzy_matcher = SkimMatcherV2::default().ignore_case();
    let mut matches_with_scores = matches
        .filter_map(|m| {
            fuzzy_matcher
                // TODO: Match description as well
                .fuzzy_match(&m.title, phrase)
                .map(|score| (m, score))
        })
        .collect::<Vec<_>>();
    matches_with_scores.sort_by_key(|item| item.1);
    matches_with_scores
        .into_iter()
        .map(|item| item.0)
        .collect::<Vec<_>>()
}

fn get_action_matches() -> Vec<Match> {
    vec![
        Match {
            title: "Lock screen".into(),
            icon: ROption::RSome("system-lock-screen".into()),
            use_pango: false,
            description: ROption::RSome("Lock the screen".into()),
            id: ROption::RSome(PowerAction::Lock.into()),
        },
        Match {
            title: "Log out".into(),
            icon: ROption::RSome("system-log-out".into()),
            use_pango: false,
            description: ROption::RSome("Log out from the session".into()),
            id: ROption::RSome(PowerAction::Logout.into()),
        },
        Match {
            title: "Poweroff".into(),
            icon: ROption::RSome("system-shutdown".into()),
            use_pango: false,
            description: ROption::RSome("Poweroff the system".into()),
            id: ROption::RSome(PowerAction::Poweroff.into()),
        },
        Match {
            title: "Reboot".into(),
            icon: ROption::RSome("system-reboot".into()),
            use_pango: false,
            description: ROption::RSome("Reboot the system".into()),
            id: ROption::RSome(PowerAction::Reboot.into()),
        },
        Match {
            title: "Suspend".into(),
            icon: ROption::RSome("system-suspend".into()),
            use_pango: false,
            description: ROption::RSome("Suspend the system".into()),
            id: ROption::RSome(PowerAction::Suspend.into()),
        },
        Match {
            title: "Hibernate".into(),
            icon: ROption::RSome("system-suspend-hibernate".into()),
            use_pango: false,
            description: ROption::RSome("Hibernate the system".into()),
            id: ROption::RSome(PowerAction::Hibernate.into()),
        },
    ]
}

fn get_confirm_matches() -> Vec<Match> {
    // TODO: Parametrize with the pending action for better message
    vec![
        Match {
            title: "Confirm".into(),
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

#[handler]
fn handler(selection: Match, state: &mut State) -> HandleResult {
    if let Some(ref pending_action) = state.pending_action {
        let confirm_action = ConfirmAction::try_from(selection.id.unwrap()).unwrap();

        if confirm_action == ConfirmAction::Confirm {
            let power_action_config = state.config.get_action_config(pending_action);
            execute_power_action(power_action_config);
            state.pending_action = None;
            HandleResult::Refresh(false)
        } else {
            HandleResult::Close
        }
    } else {
        let power_action = PowerAction::try_from(selection.id.unwrap()).unwrap();
        let power_action_config = state.config.get_action_config(&power_action);

        if power_action_config.confirm {
            state.pending_action = Some(power_action);
            HandleResult::Refresh(true)
        } else {
            execute_power_action(power_action_config);
            HandleResult::Close
        }
    }
}

fn execute_power_action(action: &PowerActionConfig) {
    // TODO: Implement the command execution
    todo!("test")
}
