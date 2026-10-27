//! Cosmetic skin catalog. Traits are indexes into a fixed list. There is no
//! string, URL, or upload. A skin changes nothing about scoring or caps.

use serde::{Deserialize, Serialize};

use crate::ONE_NEAR;

/// What a mint keeps. Storage staking comes out of this; it is not a second charge.
pub const MINT_PRICE_YOCTO: u128 = ONE_NEAR;

pub const BODY_STYLES: u8 = 8;
pub const FACE_STYLES: u8 = 4;
pub const ROLE_OUTFITS: u8 = 6;
pub const HAT_STYLES: u8 = 6;
pub const ACCESSORIES: u8 = 6;
pub const TRAILS: u8 = 4;

/// Allowlisted indexes. The renderer maps these onto GLB parts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub struct SkinTraits {
    pub body: u8,
    pub face: u8,
    pub role: u8,
    pub hat: u8,
    pub accessory: u8,
    pub trail: u8,
}

/// `None` when every index is in the catalog.
pub fn unknown_trait(traits: &SkinTraits) -> Option<&'static str> {
    if traits.body >= BODY_STYLES
        || traits.face >= FACE_STYLES
        || traits.role >= ROLE_OUTFITS
        || traits.hat >= HAT_STYLES
        || traits.accessory >= ACCESSORIES
        || traits.trail >= TRAILS
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
        body: (acc % u32::from(BODY_STYLES)) as u8,
        face: ((acc >> 3) % u32::from(FACE_STYLES)) as u8,
        role: ((acc >> 6) % u32::from(ROLE_OUTFITS)) as u8,
        hat: ((acc >> 10) % u32::from(HAT_STYLES)) as u8,
        accessory: ((acc >> 14) % u32::from(ACCESSORIES)) as u8,
        trail: ((acc >> 18) % u32::from(TRAILS)) as u8,
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
                scout.body,
                scout.face,
                scout.role,
                scout.hat,
                scout.accessory,
                scout.trail
            ),
            (0, 2, 5, 4, 4, 2)
        );
    }

    #[test]
    fn unknown_trait_is_named() {
        let mut traits = default_skin("a.fe2vi.testnet");
        traits.role = ROLE_OUTFITS;
        assert_eq!(unknown_trait(&traits), Some("unknown trait"));
    }
}
