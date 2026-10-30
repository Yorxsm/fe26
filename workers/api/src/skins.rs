//! Off-chain skins. A skin is a role, paint, screen, and trail index. Unlocks
//! are point thresholds; nothing here mints a token or holds a key.

use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
    pub id: &'static str,
    pub name: &'static str,
    pub role: u8,
    pub paint: u8,
    pub screen: u8,
    pub trail: u8,
    pub rarity: &'static str,
    pub unlock_points: u32,
    pub description: &'static str,
}

pub const SKINS: &[Skin] = &[
    Skin {
        id: "ingot",
        name: "Ingot",
        role: 0,
        paint: 0,
        screen: 0,
        trail: 0,
        rarity: "Common",
        unlock_points: 0,
        description: "The starting body. Steel paint, amber screen, sparks.",
    },
    Skin {
        id: "safecracker",
        name: "Safecracker",
        role: 0,
        paint: 6,
        screen: 3,
        trail: 0,
        rarity: "Common",
        unlock_points: 50,
        description: "Rust paint and a signal screen. Unlocked by playing the vault.",
    },
    Skin {
        id: "trader",
        name: "Trader",
        role: 1,
        paint: 1,
        screen: 1,
        trail: 1,
        rarity: "Rare",
        unlock_points: 100,
        description: "Violet paint, magenta screen, coin trail.",
    },
    Skin {
        id: "hacker",
        name: "Hacker",
        role: 2,
        paint: 2,
        screen: 2,
        trail: 2,
        rarity: "Rare",
        unlock_points: 150,
        description: "Night paint, cyan screen, glitch trail.",
    },
    Skin {
        id: "auditor",
        name: "Auditor",
        role: 3,
        paint: 3,
        screen: 6,
        trail: 3,
        rarity: "Rare",
        unlock_points: 200,
        description: "Paper paint, white screen, smoke trail.",
    },
    Skin {
        id: "builder",
        name: "Builder",
        role: 4,
        paint: 4,
        screen: 4,
        trail: 0,
        rarity: "Rare",
        unlock_points: 250,
        description: "Copper paint, mint screen, sparks.",
    },
    Skin {
        id: "scout",
        name: "Scout",
        role: 5,
        paint: 5,
        screen: 5,
        trail: 2,
        rarity: "Rare",
        unlock_points: 300,
        description: "Olive paint, lime screen, glitch trail.",
    },
    Skin {
        id: "gold-leaf",
        name: "Gold leaf",
        role: 0,
        paint: 8,
        screen: 0,
        trail: 1,
        rarity: "Mythic",
        unlock_points: 500,
        description: "Gold metal, amber screen, coin trail.",
    },
    Skin {
        id: "season-crown",
        name: "Season crown",
        role: 3,
        paint: 9,
        screen: 6,
        trail: 1,
        rarity: "Mythic",
        unlock_points: 750,
        description: "Chrome finish, white screen, coin trail.",
    },
];

pub fn catalog() -> &'static [Skin] {
    SKINS
}

pub fn find(id: &str) -> Option<&'static Skin> {
    SKINS.iter().find(|skin| skin.id == id)
}

pub fn default_skin() -> &'static Skin {
    &SKINS[0]
}

pub fn skin_json(skin: &Skin) -> serde_json::Value {
    serde_json::json!({
        "id": skin.id,
        "name": skin.name,
        "role": skin.role,
        "paint": skin.paint,
        "screen": skin.screen,
        "trail": skin.trail,
        "rarity": skin.rarity,
        "unlockPoints": skin.unlock_points,
        "description": skin.description,
    })
}
