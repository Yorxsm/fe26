//! Cosmetic skin catalog. Traits are indexes into a fixed list. There is no
//! string, URL, or upload. A skin changes nothing about scoring or caps.

use serde::{Deserialize, Serialize};

use crate::ONE_NEAR;

/// What a mint keeps. Storage staking comes out of this; it is not a second charge.
pub const MINT_PRICE_YOCTO: u128 = ONE_NEAR;

pub const PAINTS: u8 = 12;
pub const SCREENS: u8 = 8;
pub const EYE_STYLES: u8 = 6;
pub const HEADWEAR: u8 = 8;
pub const FACE_ITEMS: u8 = 4;
pub const BACK_ITEMS: u8 = 4;
pub const EMBLEMS: u8 = 6;
pub const TRAILS: u8 = 4;
pub const ROLE_OUTFITS: u8 = 6;

/// Allowlisted indexes. The renderer maps these onto GLB parts one to one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct SkinTraits {
    pub paint: u8,
    pub screen: u8,
    pub eyes: u8,
    pub headwear: u8,
    pub face_item: u8,
    pub back_item: u8,
    pub emblem: u8,
    pub trail: u8,
    pub role: u8,
}

/// `None` when every index is in the catalog.
pub fn unknown_trait(traits: &SkinTraits) -> Option<&'static str> {
    if traits.paint >= PAINTS
        || traits.screen >= SCREENS
        || traits.eyes >= EYE_STYLES
        || traits.headwear >= HEADWEAR
        || traits.face_item >= FACE_ITEMS
        || traits.back_item >= BACK_ITEMS
        || traits.emblem >= EMBLEMS
        || traits.trail >= TRAILS
        || traits.role >= ROLE_OUTFITS
    {
        Some("unknown trait")
    } else {
        None
    }
}

/// Free look derived from the account id. `apps/web/lib/skin.ts` uses this mix.
/// Calling this does not store anything and does not cost NEAR.
pub fn default_skin(account_id: &str) -> SkinTraits {
    let mut acc: u32 = 2_166_136_261;
    for byte in account_id.as_bytes() {
        acc ^= u32::from(*byte);
        acc = acc.wrapping_mul(16_777_619);
    }
    SkinTraits {
        paint: (acc % u32::from(PAINTS)) as u8,
        screen: ((acc >> 4) % u32::from(SCREENS)) as u8,
        eyes: ((acc >> 7) % u32::from(EYE_STYLES)) as u8,
        headwear: ((acc >> 10) % u32::from(HEADWEAR)) as u8,
        face_item: ((acc >> 13) % u32::from(FACE_ITEMS)) as u8,
        back_item: ((acc >> 15) % u32::from(BACK_ITEMS)) as u8,
        emblem: ((acc >> 17) % u32::from(EMBLEMS)) as u8,
        trail: ((acc >> 20) % u32::from(TRAILS)) as u8,
        role: ((acc >> 22) % u32::from(ROLE_OUTFITS)) as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_skin_is_free_and_in_catalog() {
        let first = default_skin("scout.vault.fe2vi.testnet");
        let again = default_skin("scout.vault.fe2vi.testnet");
        assert_eq!(first, again);
        assert!(unknown_trait(&first).is_none());
        assert!(unknown_trait(&default_skin("pike.war.fe2vi.testnet")).is_none());
        let scout = default_skin("scout.vault.fe2vi.testnet");
        assert_eq!(
            (
                scout.paint,
                scout.screen,
                scout.eyes,
                scout.headwear,
                scout.face_item,
                scout.back_item,
                scout.emblem,
                scout.trail,
                scout.role,
            ),
            (8, 7, 5, 0, 0, 0, 2, 0, 2)
        );
    }

    #[test]
    fn unknown_trait_is_named() {
        let mut traits = default_skin("a.fe2vi.testnet");
        traits.role = ROLE_OUTFITS;
        assert_eq!(unknown_trait(&traits), Some("unknown trait"));
        traits.role = 0;
        traits.paint = PAINTS;
        assert_eq!(unknown_trait(&traits), Some("unknown trait"));
    }

    #[test]
    fn catalog_matches_the_trait_set() {
        assert_eq!(
            (PAINTS, SCREENS, EYE_STYLES, HEADWEAR, FACE_ITEMS, BACK_ITEMS, EMBLEMS, TRAILS),
            (12, 8, 6, 8, 4, 4, 6, 4)
        );
    }
}
