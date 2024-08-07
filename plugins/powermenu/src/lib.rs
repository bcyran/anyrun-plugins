#![allow(clippy::needless_pass_by_value, clippy::wildcard_imports)]
use std::{fmt::Display, fs};

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

impl Display for PowerAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display_name = match self {
            PowerAction::Lock => "Lock the screen",
            PowerAction::Logout => "Log out",
            PowerAction::Poweroff => "Power off",
            PowerAction::Reboot => "Reboot",
            PowerAction::Suspend => "Suspend",
            PowerAction::Hibernate => "Hibernate",
        };
        write!(f, "{}", display_name)
    }
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
    let config = fs::read_to_string(format!("{config_dir}/powermenu.ron")).map_or_else(
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
        name: "Power menu".into(),
        icon: "computer".into(),
    }
}

#[get_matches]
fn get_matches(input: RString, state: &State) -> RVec<Match> {
    state
        .pending_action
        .as_ref()
        .map_or_else(
            || get_fuzzy_matches(get_action_matches().into_iter(), &input),
            get_confirm_matches,
        )
        .into()
}

fn get_fuzzy_matches(matches: impl Iterator<Item = Match>, phrase: &str) -> Vec<Match> {
    let fuzzy_matcher = SkimMatcherV2::default().ignore_case();
    let mut matches_with_scores = matches
        .filter_map(|m| get_match_score(&fuzzy_matcher, &m, phrase).map(|score| (m, score)))
        .collect::<Vec<_>>();
    matches_with_scores.sort_by_key(|item| item.1);
    matches_with_scores
        .into_iter()
        .map(|item| item.0)
        .collect::<Vec<_>>()
}

fn get_match_score(
    fuzzy_matcher: &impl FuzzyMatcher,
    match_to_score: &Match,
    phrase: &str,
) -> Option<i64> {
    let maybe_title_score = fuzzy_matcher.fuzzy_match(&match_to_score.title, phrase);
    let maybe_description_score = match_to_score
        .description
        .as_ref()
        .and_then(|desc| fuzzy_matcher.fuzzy_match(desc, phrase).into())
        .into();

    maybe_title_score.max(maybe_description_score)
}

fn get_action_matches() -> Vec<Match> {
    vec![
        Match {
            title: PowerAction::Lock.to_string().into(),
            icon: ROption::RSome("system-lock-screen".into()),
            use_pango: false,
            description: ROption::RSome("Lock the screen".into()),
            id: ROption::RSome(PowerAction::Lock.into()),
        },
        Match {
            title: PowerAction::Logout.to_string().into(),
            icon: ROption::RSome("system-log-out".into()),
            use_pango: false,
            description: ROption::RSome("Log out from the session".into()),
            id: ROption::RSome(PowerAction::Logout.into()),
        },
        Match {
            title: PowerAction::Poweroff.to_string().into(),
            icon: ROption::RSome("system-shutdown".into()),
            use_pango: false,
            description: ROption::RSome("Poweroff the system".into()),
            id: ROption::RSome(PowerAction::Poweroff.into()),
        },
        Match {
            title: PowerAction::Reboot.to_string().into(),
            icon: ROption::RSome("system-reboot".into()),
            use_pango: false,
            description: ROption::RSome("Reboot the system".into()),
            id: ROption::RSome(PowerAction::Reboot.into()),
        },
        Match {
            title: PowerAction::Suspend.to_string().into(),
            icon: ROption::RSome("system-suspend".into()),
            use_pango: false,
            description: ROption::RSome("Suspend the system".into()),
            id: ROption::RSome(PowerAction::Suspend.into()),
        },
        Match {
            title: PowerAction::Hibernate.to_string().into(),
            icon: ROption::RSome("system-suspend-hibernate".into()),
            use_pango: false,
            description: ROption::RSome("Hibernate the system".into()),
            id: ROption::RSome(PowerAction::Hibernate.into()),
        },
    ]
}

fn get_confirm_matches(action_to_confirm: &PowerAction) -> Vec<Match> {
    vec![
        Match {
            title: action_to_confirm.to_string().into(),
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
