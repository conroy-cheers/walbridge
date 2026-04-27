use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetScope {
    RuntimePortable,
    SystemLevel,
    SpecialCase,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PortStatus {
    Implemented,
    Planned,
    OutOfScope,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct TargetSupport {
    pub name: &'static str,
    pub scope: TargetScope,
    pub status: PortStatus,
    pub reason: &'static str,
}

const IMPLEMENTED_RUNTIME_TARGETS: &[TargetSupport] = &[
    TargetSupport {
        name: "gtk",
        scope: TargetScope::RuntimePortable,
        status: PortStatus::Implemented,
        reason: "Runtime-generated GTK themes with live theme-name flips.",
    },
    TargetSupport {
        name: "qt",
        scope: TargetScope::RuntimePortable,
        status: PortStatus::Implemented,
        reason: "Runtime-generated qtct palettes and config.",
    },
    TargetSupport {
        name: "ghostty",
        scope: TargetScope::RuntimePortable,
        status: PortStatus::Implemented,
        reason: "Runtime-generated Ghostty theme payload using the `walbridge` theme name.",
    },
    TargetSupport {
        name: "bat",
        scope: TargetScope::RuntimePortable,
        status: PortStatus::Implemented,
        reason: "Runtime-generated bat theme plus cache rebuild using the `walbridge` theme name.",
    },
    TargetSupport {
        name: "fish",
        scope: TargetScope::RuntimePortable,
        status: PortStatus::Implemented,
        reason: "Runtime-generated Fish color snippet sourced by interactive shells.",
    },
    TargetSupport {
        name: "vscode",
        scope: TargetScope::RuntimePortable,
        status: PortStatus::Implemented,
        reason: "Runtime-generated VS Code/VSCodium theme extension.",
    },
    TargetSupport {
        name: "starship",
        scope: TargetScope::RuntimePortable,
        status: PortStatus::Implemented,
        reason: "Runtime-generated Starship config derived from the static template and using the `walbridge` palette.",
    },
    TargetSupport {
        name: "wezterm",
        scope: TargetScope::RuntimePortable,
        status: PortStatus::Implemented,
        reason: "Runtime-generated WezTerm sidecar config fragment using the `walbridge` color scheme.",
    },
    TargetSupport {
        name: "btop",
        scope: TargetScope::RuntimePortable,
        status: PortStatus::Implemented,
        reason: "Runtime-generated btop theme file.",
    },
];

const PLANNED_RUNTIME_TARGET_NAMES: &[&str] = &[
    "alacritty",
    "anki",
    "ashell",
    "avizo",
    "bemenu",
    "blender",
    "broot",
    "bspwm",
    "cava",
    "cavalier",
    "dank-material-shell",
    "discord",
    "dunst",
    "emacs",
    "eog",
    "fcitx5",
    "feh",
    "firefox",
    "fnott",
    "foliate",
    "foot",
    "forge",
    "fuzzel",
    "fzf",
    "gdu",
    "gedit",
    "gitui",
    "glance",
    "gnome-text-editor",
    "gtksourceview",
    "halloy",
    "helix",
    "hyprland",
    "hyprlock",
    "hyprpanel",
    "hyprpaper",
    "i3",
    "i3bar-river",
    "i3status-rust",
    "jjui",
    "k9s",
    "kitty",
    "kubecolor",
    "lazygit",
    "mako",
    "mangohud",
    "micro",
    "mpv",
    "ncspot",
    "noctalia-shell",
    "nushell",
    "obsidian",
    "opencode",
    "qutebrowser",
    "rio",
    "river",
    "rofi",
    "sioyek",
    "spotify-player",
    "sway",
    "swaylock",
    "swaync",
    "sxiv",
    "tmux",
    "tofi",
    "vicinae",
    "vivid",
    "waybar",
    "wayfire",
    "wayprompt",
    "wob",
    "wofi",
    "wpaperd",
    "xfce",
    "xresources",
    "yazi",
    "zathura",
    "zed",
    "zellij",
    "zen-browser",
];

const SYSTEM_LEVEL_TARGETS: &[TargetSupport] = &[
    TargetSupport {
        name: "chromium",
        scope: TargetScope::SystemLevel,
        status: PortStatus::OutOfScope,
        reason: "Stylix only sets Chromium policy hints through the NixOS module.",
    },
    TargetSupport {
        name: "console",
        scope: TargetScope::SystemLevel,
        status: PortStatus::OutOfScope,
        reason: "Virtual console theming is system-level, not user-session runtime config.",
    },
    TargetSupport {
        name: "grub",
        scope: TargetScope::SystemLevel,
        status: PortStatus::OutOfScope,
        reason: "Bootloader theming must be built into the system image.",
    },
    TargetSupport {
        name: "jankyborders",
        scope: TargetScope::SystemLevel,
        status: PortStatus::OutOfScope,
        reason: "Darwin-specific system integration does not belong in walbridge.",
    },
    TargetSupport {
        name: "kmscon",
        scope: TargetScope::SystemLevel,
        status: PortStatus::OutOfScope,
        reason: "Console compositor theming is system-level, not user-session runtime config.",
    },
    TargetSupport {
        name: "lightdm",
        scope: TargetScope::SystemLevel,
        status: PortStatus::OutOfScope,
        reason: "Display manager theming must be deployed through the system configuration.",
    },
    TargetSupport {
        name: "limine",
        scope: TargetScope::SystemLevel,
        status: PortStatus::OutOfScope,
        reason: "Bootloader theming must be built into the system image.",
    },
    TargetSupport {
        name: "plymouth",
        scope: TargetScope::SystemLevel,
        status: PortStatus::OutOfScope,
        reason: "Boot splash theming must be built into the system image.",
    },
    TargetSupport {
        name: "regreet",
        scope: TargetScope::SystemLevel,
        status: PortStatus::OutOfScope,
        reason: "Login greeter theming is owned by the display manager configuration.",
    },
];

const SPECIAL_CASE_TARGETS: &[TargetSupport] = &[
    TargetSupport {
        name: "font-packages",
        scope: TargetScope::SpecialCase,
        status: PortStatus::Planned,
        reason: "Fonts need Home Manager packaging and app wiring, not just runtime file generation.",
    },
    TargetSupport {
        name: "fontconfig",
        scope: TargetScope::SpecialCase,
        status: PortStatus::Planned,
        reason: "Fontconfig is runtime-portable but needs ownership of shared fontconfig snippets.",
    },
    TargetSupport {
        name: "gnome",
        scope: TargetScope::SpecialCase,
        status: PortStatus::Planned,
        reason: "GNOME shell theming needs DConf, extension management, and session hooks.",
    },
    TargetSupport {
        name: "kde",
        scope: TargetScope::SpecialCase,
        status: PortStatus::Planned,
        reason: "KDE support needs runtime files plus Plasma/KConfig session integration.",
    },
    TargetSupport {
        name: "neovim",
        scope: TargetScope::SpecialCase,
        status: PortStatus::Planned,
        reason: "Neovim spans several ecosystems and likely needs editor-specific installation hooks.",
    },
    TargetSupport {
        name: "spicetify",
        scope: TargetScope::SpecialCase,
        status: PortStatus::Planned,
        reason: "Spotify theming depends on the spicetify patch pipeline rather than simple file writes.",
    },
];

pub fn stylix_inventory() -> Vec<TargetSupport> {
    let mut targets = Vec::new();
    targets.extend_from_slice(IMPLEMENTED_RUNTIME_TARGETS);

    for name in PLANNED_RUNTIME_TARGET_NAMES {
        targets.push(TargetSupport {
            name,
            scope: TargetScope::RuntimePortable,
            status: PortStatus::Planned,
            reason: "Runtime-portable Stylix target not yet ported into walbridge.",
        });
    }

    targets.extend_from_slice(SYSTEM_LEVEL_TARGETS);
    targets.extend_from_slice(SPECIAL_CASE_TARGETS);
    targets.sort_by(|a, b| a.name.cmp(b.name));
    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_contains_known_targets() {
        let targets = stylix_inventory();
        assert!(targets.iter().any(|target| target.name == "gtk"));
        assert!(targets.iter().any(|target| target.name == "chromium"));
        assert!(targets.iter().any(|target| target.name == "wezterm"));
    }

    #[test]
    fn inventory_has_unique_names() {
        let targets = stylix_inventory();
        let mut names: Vec<_> = targets.iter().map(|target| target.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), targets.len());
    }
}
