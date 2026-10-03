//! Workarounds for DayZ engine bugs, one switch each.
//!
//! Nothing here adds a feature. Every patch exists because the engine does something that
//! crashes or misbehaves, each one is a setting that can be turned off, and each one says in
//! the log what it did. A workaround nobody can switch off and nobody can count is
//! indistinguishable from a new bug.

mod prepared_view;

use dayz_plugin_sdk::{export_plugin, Host, Plugin, PluginError, Setting};

/// Setting that switches the prepared-view guard.
const GUARD_SETTING: &str = "guard_prepared_view";

struct Patches;

impl Plugin for Patches {
    const NAME: &'static str = "patches";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const DESCRIPTION: &'static str = "Works around engine bugs; each patch has its own switch.";

    fn start(host: Host) -> Result<Self, PluginError> {
        host.setting(
            &Setting::bool(GUARD_SETTING, "Guard the render view", true).with_description(
                "Skip a view the engine would render without having prepared one. Without \
                     this the game can crash after its window is dragged.",
            ),
        )?;
        host.command(
            "status",
            "Show which patches are on and how often each has acted.",
            "",
        )?;
        // Unloaded rather than crashing later: a build whose database has no execute_view is
        // a build this plugin cannot guard, and saying so is the whole value of asking.
        host.require_symbols(["render.execute_view"])?;
        let enabled = host.get_as::<bool>(GUARD_SETTING).unwrap_or(true);
        prepared_view::install(&host, enabled)?;
        Ok(Patches)
    }

    fn on_command(&self, host: &Host, name: &str, _args: &str) -> Result<(), PluginError> {
        if name != "status" {
            return Err(PluginError::Unsupported);
        }
        let (forwarded, skipped, enabled) = prepared_view::counters();
        host.console_print(&format!(
            "prepared view guard: {}, {skipped} skipped, {forwarded} forwarded",
            if enabled { "on" } else { "off" }
        ));
        Ok(())
    }

    fn on_setting_changed(&self, _host: &Host, key: &str, value: &str) {
        if key == GUARD_SETTING {
            prepared_view::set_enabled(value == "true");
            log::info!("prepared view guard is now {value}");
        }
    }
}

export_plugin!(Patches);
